use crate::cli::StartupState;
use crate::core::column_stats::{
    COLUMN_STATS_BUFFER_COLS, COLUMN_STATS_RECALC_MARGIN_COLS, ColumnStats, ColumnStatsRequest,
    apply_positional_updates,
};
use crate::core::command::{CoreAction, DiffMode};
use crate::core::data::SequenceRecord;
use crate::core::parser::{self, Alignment, SequenceType};
use crate::core::{AlignmentData, Viewport};
use regex::Regex;
use std::sync::Arc;
use tracing::debug;

/// Represents the current loading status of the application, including any error messages if
/// loading has failed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LoadingState {
    #[default]
    Idle,
    Loaded,
    Failed(String),
}

impl std::fmt::Display for LoadingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadingState::Idle => write!(f, "Status: Idle"),
            LoadingState::Loaded => write!(f, "Status: Loaded"),
            LoadingState::Failed(_) => write!(f, "Status: Failed"),
        }
    }
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
    pub input_path: Option<String>,
    pub initial_position: usize,
    pub reference_sequence_id: Option<usize>,
    pub pinned_sequence_ids: Vec<usize>,
    pub display_sequence_ids: Vec<usize>,
    pub display_pinned_count: usize,
    pub diff_mode: DiffMode,
    pub translate_nucleotide_to_amino_acid: bool,
    pub translation_frame: u8,
    pub consensus_method: crate::core::column_stats::ConsensusMethod,
    pub consensus: Option<Vec<u8>>,
    pub conservation: Option<Vec<f32>>,
    // will contain the current window + a buffer either side, or None if no window is currently cached
    pub column_stats_window: Option<(usize, usize)>,
}

impl CoreState {
    #[must_use]
    pub fn new(startup: StartupState) -> Self {
        let input_path = startup.file_path.clone();
        let data = AlignmentData::default();

        Self {
            data,
            viewport: Viewport::default(),
            filter_text: String::new(),
            filter_regex: None,
            loading_state: LoadingState::default(),
            input_path,
            initial_position: startup.initial_position,
            reference_sequence_id: None,
            pinned_sequence_ids: Vec::new(),
            display_sequence_ids: Vec::new(),
            display_pinned_count: 0,
            diff_mode: DiffMode::Off,
            translate_nucleotide_to_amino_acid: false,
            translation_frame: 0,
            consensus_method: crate::core::column_stats::ConsensusMethod::MajorityNonGap,
            consensus: None,
            conservation: None,
            column_stats_window: None,
        }
    }

    #[must_use]
    pub fn data(&self) -> &AlignmentData {
        &self.data
    }

    /// Applies a single [`CoreAction`] command, where a core action is something that manipulates
    /// the application state.
    pub fn apply_action(&mut self, action: CoreAction) {
        match action {
            CoreAction::ScrollDown { amount } => {
                self.viewport.scroll_down(amount);
            }
            CoreAction::ScrollUp { amount } => {
                self.viewport.scroll_up(amount);
            }
            CoreAction::ScrollLeft { amount } => {
                self.viewport.scroll_left(amount);
            }
            CoreAction::ScrollRight { amount } => {
                self.viewport.scroll_right(amount);
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
            }
            CoreAction::JumpToPosition(position) => {
                self.viewport.jump_to_position(position);
            }
            CoreAction::ClearReference => {
                self.reference_sequence_id = None;
                self.refresh_viewport();
            }
            CoreAction::SetReference(sequence_id) => {
                self.remove_pin(sequence_id);
                self.reference_sequence_id =
                    self.data.sequences.get(sequence_id).map(|_| sequence_id);
                self.refresh_viewport();
            }
            CoreAction::PinSequence(sequence_id) => {
                if self.data.sequences.get(sequence_id).is_some()
                    && !self.is_sequence_pinned(sequence_id)
                    && self.reference_sequence_id != Some(sequence_id)
                {
                    self.pinned_sequence_ids.push(sequence_id);
                    self.refresh_viewport();
                }
            }
            CoreAction::UnpinSequence(sequence_id) => {
                self.remove_pin(sequence_id);
                self.refresh_viewport();
            }
            CoreAction::SetConsensusMethod(method) => {
                if self.consensus_method != method {
                    self.consensus_method = method;
                }
            }
            CoreAction::SetSequenceType(sequence_type) => {
                if self.data.sequence_type != Some(sequence_type) {
                    self.data.sequence_type = Some(sequence_type);
                    if sequence_type != SequenceType::Dna {
                        self.translate_nucleotide_to_amino_acid = false;
                    }
                }
            }
            CoreAction::SetTranslationFrame(frame) => {
                if self.is_dna_sequence_type() {
                    let next_frame = frame - 1;
                    if self.translation_frame != next_frame {
                        self.translation_frame = next_frame;
                    }
                }
            }
            CoreAction::ClearFilter => {
                self.apply_filter(None);
            }
            CoreAction::SetFilter { pattern, regex } => {
                self.apply_filter(Some((pattern, regex)));
            }
            CoreAction::SetDiffMode(mode) => {
                self.diff_mode = mode;
            }
            CoreAction::ToggleTranslationView => {
                self.toggle_translation_view();
            }
        }
    }

    /// Handles completion of a pending alignment load.
    pub fn handle_alignments_loaded(&mut self, result: Result<Vec<Alignment>, String>) {
        match result {
            Ok(alignments) => {
                let detected_sequence_type = parser::detect_sequence_type(&alignments);
                self.data.load_alignments(alignments);
                self.loading_state = LoadingState::Loaded;
                self.reference_sequence_id = None;
                self.pinned_sequence_ids.clear();
                self.refresh_viewport();
                self.viewport.jump_to_position(self.initial_position);

                self.data.sequence_type = Some(detected_sequence_type);
                if detected_sequence_type != SequenceType::Dna {
                    self.translate_nucleotide_to_amino_acid = false;
                }
            }
            Err(error) => {
                self.loading_state = LoadingState::Failed(error);
            }
        }
    }

    /// Applies column stats to the cache.
    pub fn apply_column_stats(&mut self, stats: ColumnStats) {
        apply_positional_updates(
            &mut self.consensus,
            self.data.sequence_length,
            b' ',
            &stats.consensus,
        );
        apply_positional_updates(
            &mut self.conservation,
            self.data.sequence_length,
            f32::NAN,
            &stats.conservation,
        );
    }

    /// Updates viewport sizing.
    ///
    /// May kick off consensus and conservation jobs if the new viewport invalidates
    /// current cached coverage windows.
    pub fn update_viewport_dimensions(
        &mut self,
        visible_width: usize,
        visible_height: usize,
        name_visible_width: usize,
    ) {
        self.viewport
            .update_dimensions(visible_width, visible_height, name_visible_width);
        self.refresh_viewport();
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
        // none = clear filter
        let Some((pattern, regex)) = filter else {
            self.filter_text.clear();
            self.filter_regex = None;
            self.refresh_viewport();
            return;
        };

        // pattern is empty = also clear filter
        // this is for UX reasons, so users can clear the filter by deleting the pattern as well
        // as using `clear-filter` command
        if pattern.is_empty() {
            self.filter_text.clear();
            self.filter_regex = None;
        } else {
            self.filter_text = pattern;
            self.filter_regex = Some(regex);
        }

        self.refresh_viewport();
    }

    /// Returns true if the viewport is close enough to a cached window edge that new positions
    /// should be computed.
    #[must_use]
    pub fn viewport_crossed_margin(&self) -> bool {
        let Some((current_start, current_end)) = self.column_stats_window else {
            return true;
        };
        let sequence_length = self.data.sequence_length;
        let margin = COLUMN_STATS_RECALC_MARGIN_COLS;
        let viewport_range = self.viewport.window().col_range;

        if current_start > 0 && viewport_range.start < current_start.saturating_add(margin) {
            return true;
        }

        current_end < sequence_length && viewport_range.end > current_end.saturating_sub(margin)
    }

    /// Clears cached consensus and conservation stats.
    ///
    /// On next pass, new stats will be calculated for the whole window.
    pub fn invalidate_column_stats(&mut self) {
        self.consensus = None;
        self.conservation = None;
        self.column_stats_window = None;
    }

    #[must_use]
    pub fn sequence_type(&self) -> SequenceType {
        self.data.sequence_type.unwrap_or(SequenceType::Dna)
    }

    /// Builds a column stats request for all uncached positions in the
    /// current viewport window.
    ///
    /// Returns a request with empty positions if all positions are already
    /// cached.
    pub fn build_column_stats_request(&mut self) -> ColumnStatsRequest {
        let sequence_length = self.data.sequence_length;
        let viewport_range = self.viewport.window().col_range;
        let window_start = viewport_range
            .start
            .saturating_sub(COLUMN_STATS_BUFFER_COLS);
        let window_end = (viewport_range.end + COLUMN_STATS_BUFFER_COLS).min(sequence_length);

        self.column_stats_window = Some((window_start, window_end));

        let positions = (window_start..window_end)
            .filter(|&position| self.position_needs_calculation(position))
            .collect::<Vec<_>>();

        debug!(
            position_count = positions.len(),
            method = ?self.consensus_method,
            sequence_type = ?self.sequence_type(),
            column_stats_window = ?self.column_stats_window,
            "Built column stats request"
        );

        let visible_sequences: Vec<SequenceRecord> = self.visible_sequences().cloned().collect();
        let sequence_type = self.sequence_type();

        ColumnStatsRequest {
            sequences: Arc::new(visible_sequences),
            positions,
            method: self.consensus_method,
            sequence_type,
        }
    }

    /// Returns true if the given position is not currently cached in consensus or conservation
    fn position_needs_calculation(&self, position: usize) -> bool {
        // the consensus and conservation caches are always calculated together, so its fine
        // just to check one to trigger both
        self.consensus
            .as_deref()
            .and_then(|consensus| consensus.get(position))
            .is_none_or(|&byte| byte == b' ')
    }

    /// Returns whether the loaded data is currently classified as DNA.
    fn is_dna_sequence_type(&self) -> bool {
        self.data.sequence_type == Some(SequenceType::Dna)
    }

    /// Toggles nucleotide to amino acid rendering.
    fn toggle_translation_view(&mut self) {
        if self.is_dna_sequence_type() {
            self.translate_nucleotide_to_amino_acid = !self.translate_nucleotide_to_amino_acid;
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

        self.display_pinned_count = self.display_sequence_ids.len();

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

    #[test]
    fn pinned_sequence_stays_visible_wtih_filter() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);

        core.apply_action(CoreAction::PinSequence(0));
        core.apply_action(CoreAction::SetFilter {
            pattern: String::from("seq-b"),
            regex: Regex::new("seq-b").expect("test regex should compile"),
        });

        let visible_ids: Vec<_> = core
            .visible_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();
        assert_eq!(visible_ids, vec![0, 1]);
    }

    #[test]
    fn pinned_sequences_render_first_in_pin_order() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);

        core.apply_action(CoreAction::PinSequence(2));
        core.apply_action(CoreAction::PinSequence(0));

        let visible_ids: Vec<_> = core
            .visible_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();
        assert_eq!(visible_ids, vec![2, 0, 1]);
    }

    #[test]
    fn setting_reference_removes_existing_pin() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);

        core.apply_action(CoreAction::PinSequence(1));
        core.apply_action(CoreAction::SetReference(1));

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

        core.apply_action(CoreAction::PinSequence(0));
        core.apply_action(CoreAction::SetFilter {
            pattern: String::from("seq-b"),
            regex: regex::Regex::new("seq-b").expect("test regex should compile"),
        });
        core.apply_action(CoreAction::UnpinSequence(0));

        let visible_ids: Vec<_> = core
            .visible_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();
        assert_eq!(visible_ids, vec![1]);
    }
}
