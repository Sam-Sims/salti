use crate::cli::StartupState;
use crate::core::command::CoreAction;
use crate::core::consensus::{
    CONSENSUS_BUFFER_COLS, CONSENSUS_RECALC_MARGIN_COLS, apply_consensus_updates,
    subset_missing_positions,
};
use crate::core::data::SequenceRecord;
use crate::core::jobs::{ConsensusRequest, spawn_load_alignments_job};
use crate::core::parser::{self, Alignment, SequenceType};
use crate::core::{AlignmentData, Viewport};
use regex::Regex;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, trace, warn};

/// Represents the current loading status of the application, including any error messages if
/// loading has failed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
    Failed(String),
}

impl std::fmt::Display for LoadingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadingState::Idle => write!(f, "Status: Idle"),
            LoadingState::Loading => write!(f, "Status: Loading alignments..."),
            LoadingState::Loaded => write!(f, "Status: Loaded"),
            LoadingState::Failed(_) => write!(f, "Status: Failed"),
        }
    }
}

/// Represents asynchronous events that can affect the core application state
#[derive(Debug)]
pub enum CoreAsyncEvent {
    AlignmentsLoaded(Result<Vec<Alignment>, String>),
    ConsensusUpdated { updates: Vec<(usize, u8)> },
}

/// Represents the visible subset of the loaded sequences for display in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleSequence {
    pub sequence_id: usize,
    pub sequence_name: Arc<str>,
}

/// The main application state
#[derive(Debug)]
pub struct CoreState {
    pub data: AlignmentData,
    pub viewport: Viewport,
    pub filter_text: String,
    pub filter_regex: Option<Regex>,
    pub loading_state: LoadingState,
    pub initial_position: usize,
    pub reference_sequence_id: Option<usize>,
    pub pinned_sequence_ids: Vec<usize>,
    pub display_sequence_ids: Vec<usize>,
    pub show_reference_diff: bool,
    pub show_consensus_diff: bool,
    pub translate_nucleotide_to_amino_acid: bool,
    pub translation_frame: u8,
    pub consensus_method: crate::core::consensus::ConsensusMethod,
    pub consensus: Option<Vec<u8>>,
    pub consensus_window: Option<(usize, usize)>,
}

impl CoreState {
    #[must_use]
    pub fn new(startup: StartupState) -> Self {
        let data = AlignmentData {
            file_path: startup.file_path,
            ..AlignmentData::default()
        };

        Self {
            data,
            viewport: Viewport::default(),
            filter_text: String::new(),
            filter_regex: None,
            loading_state: LoadingState::default(),
            initial_position: startup.initial_position,
            reference_sequence_id: None,
            pinned_sequence_ids: Vec::new(),
            display_sequence_ids: Vec::new(),
            show_reference_diff: false,
            show_consensus_diff: false,
            translate_nucleotide_to_amino_acid: false,
            translation_frame: 0,
            consensus_method: crate::core::consensus::ConsensusMethod::MajorityNonGap,
            consensus: None,
            consensus_window: None,
        }
    }

    /// Applies a single [`CoreAction`] command, where a core action is something that manipulates
    /// the application state.
    pub fn apply_action(
        &mut self,
        action: CoreAction,
        async_tx: &mpsc::Sender<CoreAsyncEvent>,
        consensus_tx: &watch::Sender<Option<ConsensusRequest>>,
    ) {
        match action {
            CoreAction::ScrollDown { amount } => {
                self.viewport.scroll_down(amount);
                self.request_consensus_if_needed(consensus_tx);
            }
            CoreAction::ScrollUp { amount } => {
                self.viewport.scroll_up(amount);
                self.request_consensus_if_needed(consensus_tx);
            }
            CoreAction::ScrollLeft { amount } => {
                self.viewport.scroll_left(amount);
                self.request_consensus_if_needed(consensus_tx);
            }
            CoreAction::ScrollRight { amount } => {
                self.viewport.scroll_right(amount);
                self.request_consensus_if_needed(consensus_tx);
            }
            CoreAction::ScrollNamesLeft { amount } => {
                self.viewport.scroll_names_left(amount);
            }
            CoreAction::ScrollNamesRight { amount } => {
                self.viewport.scroll_names_right(amount);
            }
            CoreAction::JumpToSequence(sequence_id) => {
                if self
                    .visible_pinned_sequences()
                    .any(|visible_sequence| visible_sequence.sequence_id == sequence_id)
                {
                    self.viewport.jump_to_sequence(0);
                } else {
                    let unpinned_row = self
                        .visible_unpinned_sequences()
                        .position(|visible_sequence| visible_sequence.sequence_id == sequence_id);
                    if let Some(unpinned_row) = unpinned_row {
                        self.viewport.jump_to_sequence(unpinned_row);
                    }
                }
                self.request_consensus_if_needed(consensus_tx);
            }
            CoreAction::JumpToPosition(position) => {
                self.viewport.jump_to_position(position);
                self.request_consensus_if_needed(consensus_tx);
            }
            CoreAction::ClearReference => {
                debug!(
                    previous_reference_sequence_id = ?self.reference_sequence_id,
                    "clearing reference sequence"
                );
                self.reference_sequence_id = None;
                self.refresh_viewport();
            }
            CoreAction::SetReference(sequence_id) => {
                self.remove_pin(sequence_id);
                self.reference_sequence_id =
                    self.data.sequences.get(sequence_id).map(|_| sequence_id);
                if self.reference_sequence_id.is_some() {
                    debug!(sequence_id, "set reference sequence");
                } else {
                    warn!(sequence_id, "ignored set reference for unknown sequence id");
                }
                self.refresh_viewport();
            }
            CoreAction::PinSequence(sequence_id) => {
                if self.data.sequences.get(sequence_id).is_some()
                    && !self.is_sequence_pinned(sequence_id)
                    && self.reference_sequence_id != Some(sequence_id)
                {
                    self.pinned_sequence_ids.push(sequence_id);
                    debug!(
                        sequence_id,
                        pinned_count = self.pinned_sequence_ids.len(),
                        "pinned sequence"
                    );
                    self.refresh_viewport();
                } else {
                    trace!(
                        sequence_id,
                        "ignored pin request due to existing pin, missing sequence, or reference match"
                    );
                }
            }
            CoreAction::UnpinSequence(sequence_id) => {
                self.remove_pin(sequence_id);
                debug!(
                    sequence_id,
                    pinned_count = self.pinned_sequence_ids.len(),
                    "processed unpin sequence request"
                );
                self.refresh_viewport();
            }
            CoreAction::SetConsensusMethod(method) => {
                if self.consensus_method == method {
                    trace!(method = ?method, "ignored consensus method request with unchanged value");
                } else {
                    debug!(
                        previous_method = ?self.consensus_method,
                        next_method = ?method,
                        "changed consensus method"
                    );
                    self.consensus_method = method;
                    self.reset_consensus_state();
                    self.request_consensus_if_needed(consensus_tx);
                }
            }
            CoreAction::SetSequenceType(sequence_type) => {
                self.data.sequence_type = Some(sequence_type);
                if sequence_type == SequenceType::AminoAcid {
                    self.translate_nucleotide_to_amino_acid = false;
                }
                debug!(sequence_type = ?sequence_type, "set sequence type");
            }
            CoreAction::SetTranslationFrame(frame) => {
                if self.is_dna_sequence_type() {
                    let next_frame = frame - 1;
                    if self.translation_frame == next_frame {
                        trace!(
                            frame = self.translation_frame,
                            "translation frame already set"
                        );
                    } else {
                        self.translation_frame = next_frame;
                        debug!(frame = self.translation_frame, "set translation frame");
                    }
                } else {
                    trace!("ignored translation frame update because sequence type is not DNA");
                }
            }
            CoreAction::ClearFilter => {
                self.apply_filter(None);
            }
            CoreAction::SetFilter { pattern, regex } => {
                self.apply_filter(Some((pattern, regex)));
            }
            CoreAction::ToggleReferenceDiff => {
                self.show_reference_diff = !self.show_reference_diff;
                if self.show_reference_diff {
                    self.show_consensus_diff = false;
                }
                debug!(
                    show_reference_diff = self.show_reference_diff,
                    show_consensus_diff = self.show_consensus_diff,
                    "toggled reference diff mode"
                );
            }
            CoreAction::ToggleConsensusDiff => {
                self.show_consensus_diff = !self.show_consensus_diff;
                if self.show_consensus_diff {
                    self.show_reference_diff = false;
                }
                debug!(
                    show_consensus_diff = self.show_consensus_diff,
                    show_reference_diff = self.show_reference_diff,
                    "toggled consensus diff mode"
                );
            }
            CoreAction::ToggleTranslationView => {
                self.toggle_translation_view();
            }
            CoreAction::LoadAlignment { path } => {
                self.loading_state = LoadingState::Loading;
                self.data.file_path = Some(path.clone());
                info!(path = ?path, "loading alignment file");
                spawn_load_alignments_job(path, async_tx.clone());
            }
        }
    }

    /// Handles async events.
    ///
    /// `AlignmentsLoaded` signals completion of the initial data load triggered by a `LoadAlignment`
    /// action, and carries the loaded alignments or an error message. On success, this updates the
    /// core state with the loaded data.
    ///
    /// `ConsensusUpdated` signals the completion of a background consensus computation task,
    /// and updates the current consensus windows with the new results
    pub fn handle_event(
        &mut self,
        event: CoreAsyncEvent,
        consensus_tx: &watch::Sender<Option<ConsensusRequest>>,
    ) {
        match event {
            CoreAsyncEvent::AlignmentsLoaded(result) => match result {
                Ok(alignments) => {
                    let detected_sequence_type = parser::detect_sequence_type(&alignments);
                    self.data.load_alignments(alignments);
                    self.loading_state = LoadingState::Loaded;
                    self.consensus = None;
                    self.consensus_window = None;
                    self.reference_sequence_id = None;
                    self.pinned_sequence_ids.clear();
                    self.refresh_viewport();
                    self.viewport.jump_to_position(self.initial_position);

                    self.data.sequence_type = Some(detected_sequence_type);
                    if detected_sequence_type == SequenceType::AminoAcid {
                        self.translate_nucleotide_to_amino_acid = false;
                    }
                    info!(
                        sequence_count = self.data.sequences.len(),
                        sequence_length = self.data.sequence_length,
                        sequence_type = ?detected_sequence_type,
                        initial_position = self.initial_position,
                        "alignment data loaded into core state"
                    );

                    self.request_consensus_if_needed(consensus_tx);
                }
                Err(error) => {
                    debug!(error = %error, "alignment load failed in core state");
                    self.loading_state = LoadingState::Failed(error);
                }
            },
            CoreAsyncEvent::ConsensusUpdated { updates } => {
                trace!(
                    updated_positions = updates.len(),
                    "applying consensus updates"
                );
                apply_consensus_updates(&mut self.consensus, self.data.sequence_length, updates);
            }
        }
    }

    /// Updates viewport sizing.
    ///
    /// May kick off a consensus job if the new viewport invalidates the current consensus
    /// coverage window.
    pub fn update_viewport_dimensions(
        &mut self,
        visible_width: usize,
        visible_height: usize,
        name_visible_width: usize,
        consensus_tx: &watch::Sender<Option<ConsensusRequest>>,
    ) {
        self.viewport
            .update_dimensions(visible_width, visible_height, name_visible_width);
        self.refresh_viewport();
        self.request_consensus_if_needed(consensus_tx);
    }

    /// Yields all sequence records that are not marked hidden
    pub fn visible_sequences(&self) -> impl Iterator<Item = &SequenceRecord> {
        self.display_sequence_ids
            .iter()
            .filter_map(|&sequence_id| self.data.sequences.get(sequence_id))
    }

    /// Yields pinned sequences in pin order.
    ///
    /// Used for operations that need to consider all pinned sequences
    pub fn pinned_sequences(&self) -> impl Iterator<Item = &SequenceRecord> {
        self.pinned_sequence_ids
            .iter()
            .filter_map(|&sequence_id| self.data.sequences.get(sequence_id))
    }

    /// Yields visible pinned sequences in pin order.
    ///
    /// Used for operations that only consider pinned sequences that are not marked hidden
    pub fn visible_pinned_sequences(&self) -> impl Iterator<Item = &SequenceRecord> {
        self.pinned_sequence_ids.iter().filter_map(|&sequence_id| {
            self.data
                .sequences
                .get(sequence_id)
                .filter(|sequence| !sequence.hidden)
        })
    }

    /// Yields visible unpinned sequences in original alignment order.
    ///
    /// Used for operations that only consider unpinned sequences that are not marked hidden
    pub fn visible_unpinned_sequences(&self) -> impl Iterator<Item = &SequenceRecord> {
        self.data
            .sequences
            .iter()
            .filter(|sequence| !sequence.hidden && !self.is_sequence_pinned(sequence.sequence_id))
    }

    #[must_use]
    pub fn is_sequence_pinned(&self, sequence_id: usize) -> bool {
        self.pinned_sequence_ids.contains(&sequence_id)
    }

    /// Returns the alignment for the current reference sequence, if one is selected.
    #[must_use]
    pub fn reference_alignment(&self) -> Option<&Alignment> {
        self.reference_sequence_id
            .and_then(|sequence_id| self.data.sequences.get(sequence_id))
            .map(|sequence| &sequence.alignment)
    }

    /// Updates each sequence's `hidden` flag based on current reference and filter settings.
    fn apply_hide_flags(&mut self) {
        let reference_index = self.reference_sequence_id;
        let filter = self.filter_regex.as_ref();

        for sequence in Arc::make_mut(&mut self.data.sequences).iter_mut() {
            let is_reference = reference_index == Some(sequence.sequence_id);
            let is_pinned = self.pinned_sequence_ids.contains(&sequence.sequence_id);
            let excluded_by_filter = !is_pinned
                && filter.is_some_and(|regex| !regex.is_match(sequence.alignment.id.as_ref()));
            sequence.hidden = is_reference || excluded_by_filter;
        }
    }

    /// Recomputes viewport params
    ///
    /// When filter or reference settings change, the set of visible sequences changes, which affects
    /// the viewport's row mappings and scroll bounds. This method recalculates those parameters to
    /// keep the viewport state consistent with the current visibility rules.
    fn refresh_viewport(&mut self) {
        self.apply_hide_flags();
        self.rebuild_display_sequence_ids();
        self.viewport.max_size.rows = self.display_sequence_ids.len();
        self.viewport.max_size.cols = self.data.sequence_length;
        self.viewport.max_size.name_width = self.data.max_sequence_id_len;
        self.viewport.clamp_offsets();
    }

    /// Applies or clears the ID filter pattern and refreshes derived viewport state.
    fn apply_filter(&mut self, filter: Option<(String, Regex)>) {
        let Some((pattern, regex)) = filter else {
            self.filter_text.clear();
            self.filter_regex = None;
            self.refresh_viewport();
            debug!("cleared sequence filter");
            return;
        };

        if pattern.is_empty() {
            self.filter_text.clear();
            self.filter_regex = None;
        } else {
            self.filter_text = pattern;
            self.filter_regex = Some(regex);
        }

        self.refresh_viewport();
        debug!(
            has_filter = self.filter_regex.is_some(),
            filter = %self.filter_text,
            visible_sequence_count = self.display_sequence_ids.len(),
            "applied sequence filter"
        );
    }

    /// Sends a consensus recomputation request only when the current viewport needs new positions.
    ///
    /// Determines this by checking if the viewport has scrolled close enough to the edge of the
    /// existing consensus window
    pub(crate) fn request_consensus_if_needed(
        &mut self,
        consensus_tx: &watch::Sender<Option<ConsensusRequest>>,
    ) {
        let Some(positions) = self.get_consensus_pos_for_update() else {
            trace!("consensus update not required");
            return;
        };

        debug!(
            position_count = positions.len(),
            method = ?self.consensus_method,
            consensus_window = ?self.consensus_window,
            "queued consensus update request"
        );
        let request = ConsensusRequest {
            sequences: self.data.sequences.clone(),
            positions,
            method: self.consensus_method,
        };
        consensus_tx.send_replace(Some(request));
    }

    /// Determines which consensus positions require calculation
    fn get_consensus_pos_for_update(&mut self) -> Option<Vec<usize>> {
        let sequence_length = self.data.sequence_length;
        if sequence_length == 0 || self.data.sequences.is_empty() {
            trace!(
                sequence_length,
                sequence_count = self.data.sequences.len(),
                "skipping consensus update because no sequence data is loaded"
            );
            return None;
        }

        let viewport_range = self.viewport.window().col_range;
        let window_start = viewport_range.start.saturating_sub(CONSENSUS_BUFFER_COLS);
        let window_end = (viewport_range.end + CONSENSUS_BUFFER_COLS).min(sequence_length);
        let needed_window = (window_start, window_end);

        let should_update = match self.consensus_window {
            None => true,
            Some((current_start, current_end)) => {
                let viewport_start = viewport_range.start;
                let viewport_end = viewport_range.end;
                viewport_start < (current_start + CONSENSUS_RECALC_MARGIN_COLS)
                    || viewport_end > (current_end.saturating_sub(CONSENSUS_RECALC_MARGIN_COLS))
            }
        };

        if !should_update {
            trace!(
                consensus_window = ?self.consensus_window,
                viewport_start = viewport_range.start,
                viewport_end = viewport_range.end,
                "skipping consensus update because window still covers viewport"
            );
            return None;
        }

        self.consensus_window = Some(needed_window);
        let existing = self.consensus.as_deref();
        let positions_to_calculate =
            subset_missing_positions(needed_window.0, needed_window.1, existing);

        if positions_to_calculate.is_empty() {
            trace!(
                consensus_window = ?self.consensus_window,
                "skipping consensus update because all positions are already cached"
            );
            return None;
        }

        trace!(
            consensus_window = ?self.consensus_window,
            position_count = positions_to_calculate.len(),
            "consensus update required"
        );
        Some(positions_to_calculate)
    }

    /// Drops all cached consensus state.
    fn reset_consensus_state(&mut self) {
        self.consensus = None;
        self.consensus_window = None;
    }

    /// Returns whether the loaded data is currently classified as DNA.
    fn is_dna_sequence_type(&self) -> bool {
        self.data.sequence_type == Some(SequenceType::Dna)
    }

    /// Toggles nucleotide to amino acid rendering.
    fn toggle_translation_view(&mut self) {
        if self.is_dna_sequence_type() {
            self.translate_nucleotide_to_amino_acid = !self.translate_nucleotide_to_amino_acid;
            debug!(
                translate_nucleotide_to_amino_acid = self.translate_nucleotide_to_amino_acid,
                "toggled nucleotide translation view"
            );
        } else {
            trace!("ignored translation view toggle because sequence type is not DNA");
        }
    }

    /// Rebuilds the list of sequence IDs when sequence visibility changes
    ///
    /// Pinned sequences are always included at the start of the list in pin order
    /// Hidden sequences are excluded entirely.
    fn rebuild_display_sequence_ids(&mut self) {
        self.display_sequence_ids.clear();

        for &sequence_id in &self.pinned_sequence_ids {
            if let Some(sequence) = self.data.sequences.get(sequence_id)
                && !sequence.hidden
            {
                self.display_sequence_ids.push(sequence_id);
            }
        }

        for sequence in self.data.sequences.iter() {
            if !sequence.hidden && !self.is_sequence_pinned(sequence.sequence_id) {
                self.display_sequence_ids.push(sequence.sequence_id);
            }
        }
    }

    /// Removes a sequence ID from the pinned list
    fn remove_pin(&mut self, sequence_id: usize) {
        self.pinned_sequence_ids
            .retain(|&pinned_id| pinned_id != sequence_id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::{mpsc, watch};

    use super::*;
    use crate::core::parser::Alignment;

    fn test_core_with_ids(ids: &[&str]) -> CoreState {
        let alignments = ids
            .iter()
            .map(|id| Alignment {
                id: Arc::from(*id),
                sequence: Arc::from(b"ACGT".to_vec()),
            })
            .collect();

        let mut core = CoreState::new(StartupState::default());
        core.data.load_alignments(alignments);
        core.refresh_viewport();
        core
    }

    fn test_channels() -> (
        mpsc::Sender<CoreAsyncEvent>,
        watch::Sender<Option<ConsensusRequest>>,
    ) {
        let (async_tx, _async_rx) = mpsc::channel(1);
        let (consensus_tx, _consensus_rx) = watch::channel(None);
        (async_tx, consensus_tx)
    }

    #[test]
    fn pinned_sequence_stays_visible_when_filter_excludes_it() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);
        let (async_tx, consensus_tx) = test_channels();

        core.apply_action(CoreAction::PinSequence(0), &async_tx, &consensus_tx);
        core.apply_action(
            CoreAction::SetFilter {
                pattern: String::from("seq-b"),
                regex: Regex::new("seq-b").expect("test regex should compile"),
            },
            &async_tx,
            &consensus_tx,
        );

        let visible_ids: Vec<_> = core
            .visible_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();
        assert_eq!(visible_ids, vec![0, 1]);
    }

    #[test]
    fn pinned_sequences_render_first_in_pin_order() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);
        let (async_tx, consensus_tx) = test_channels();

        core.apply_action(CoreAction::PinSequence(2), &async_tx, &consensus_tx);
        core.apply_action(CoreAction::PinSequence(0), &async_tx, &consensus_tx);

        let visible_ids: Vec<_> = core
            .visible_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();
        assert_eq!(visible_ids, vec![2, 0, 1]);
    }

    #[test]
    fn setting_reference_removes_existing_pin() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);
        let (async_tx, consensus_tx) = test_channels();

        core.apply_action(CoreAction::PinSequence(1), &async_tx, &consensus_tx);
        core.apply_action(CoreAction::SetReference(1), &async_tx, &consensus_tx);

        assert!(!core.is_sequence_pinned(1));
        let visible_ids: Vec<_> = core
            .visible_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();
        assert_eq!(visible_ids, vec![0, 2]);
    }

    #[test]
    fn unpinned_sequence_returns_to_filter_behaviour() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);
        let (async_tx, consensus_tx) = test_channels();

        core.apply_action(CoreAction::PinSequence(0), &async_tx, &consensus_tx);
        core.apply_action(
            CoreAction::SetFilter {
                pattern: String::from("seq-b"),
                regex: regex::Regex::new("seq-b").expect("test regex should compile"),
            },
            &async_tx,
            &consensus_tx,
        );
        core.apply_action(CoreAction::UnpinSequence(0), &async_tx, &consensus_tx);

        let visible_ids: Vec<_> = core
            .visible_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();
        assert_eq!(visible_ids, vec![1]);
    }
}
