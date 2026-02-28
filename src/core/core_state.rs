use crate::cli::StartupState;
use crate::core::column_stats::{
    COLUMN_STATS_BUFFER_COLS, COLUMN_STATS_RECALC_MARGIN_COLS, ColumnStats, ColumnStatsRequest,
    ConsensusMethod, apply_positional_updates,
};
use crate::core::command::{CoreAction, DiffMode};
use crate::core::data::SequenceRecord;
use crate::core::parser::{self, Alignment, SequenceType};
use crate::core::visibility::Visibility;
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
    Loading,
    Loaded,
    Failed(String),
}

impl std::fmt::Display for LoadingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadingState::Idle => write!(f, "Status: Idle"),
            LoadingState::Loading => write!(f, "Status: Loading"),
            LoadingState::Loaded => write!(f, "Status: Loaded"),
            LoadingState::Failed(_) => write!(f, "Status: Failed"),
        }
    }
}

/// Filter state
#[derive(Debug, Default)]
pub struct FilterState {
    pub text: String,
    pub regex: Option<Regex>,
}

/// The main application state
#[derive(Debug)]
pub struct CoreState {
    pub data: AlignmentData,
    pub viewport: Viewport,
    pub filter: FilterState,
    pub loading_state: LoadingState,
    pub input_path: Option<String>,
    pub initial_position: usize,
    pub reference_sequence_id: Option<usize>,
    pub pinned_sequence_ids: Vec<usize>,
    pub row_visibility: Visibility,
    pub column_visibility: Visibility,
    pub diff_mode: DiffMode,
    pub translate_nucleotide_to_amino_acid: bool,
    pub translation_frame: u8,
    pub consensus_method: ConsensusMethod,
    pub consensus: Option<Vec<u8>>,
    pub conservation: Option<Vec<f32>>,
    // will contain the current window + a buffer either side, or None if no window is currently cached
    pub column_stats_window: Option<(usize, usize)>,
}

impl CoreState {
    #[must_use]
    pub fn new(startup: StartupState) -> Self {
        Self {
            data: AlignmentData::default(),
            viewport: Viewport::default(),
            filter: FilterState::default(),
            loading_state: LoadingState::default(),
            input_path: startup.file_path,
            initial_position: startup.initial_position,
            reference_sequence_id: None,
            pinned_sequence_ids: Vec::new(),
            row_visibility: Visibility::default(),
            column_visibility: Visibility::default(),
            diff_mode: DiffMode::Off,
            translate_nucleotide_to_amino_acid: false,
            translation_frame: 0,
            consensus_method: ConsensusMethod::default(),
            consensus: None,
            conservation: None,
            column_stats_window: None,
        }
    }

    /// Marks the core as loading the given input
    pub fn prepare_load(&mut self, input: String) {
        self.input_path = Some(input);
        self.loading_state = LoadingState::Loading;
    }

    /// Sets core as idle
    pub fn mark_idle(&mut self) {
        self.loading_state = LoadingState::Idle;
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
                if let Some(visible_row) = self
                    .row_visibility
                    .visible_to_absolute()
                    .iter()
                    .position(|&visible_sequence_id| visible_sequence_id == sequence_id)
                {
                    let scroll_target = visible_row.saturating_sub(self.visible_pinned_count());
                    self.viewport.jump_to_sequence(scroll_target);
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
                self.data.sequence_type = Some(detected_sequence_type);
                if detected_sequence_type != SequenceType::Dna {
                    self.translate_nucleotide_to_amino_acid = false;
                }
                self.row_visibility.reset_all(self.data.sequences.len());
                self.column_visibility.reset_all(self.data.sequence_length);
                self.loading_state = LoadingState::Loaded;
                self.reference_sequence_id = None;
                self.pinned_sequence_ids.clear();
                self.refresh_viewport();
                self.viewport.jump_to_position(self.initial_position);
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
            .set_dimensions(visible_width, visible_height, name_visible_width);
        self.refresh_viewport();
    }

    /// Yields all visible sequence records in display order.
    ///
    /// This means visible pinned sequences first in pin order, followed by visible
    /// unpinned sequences in same order as the original alignment.
    pub fn all_visible_sequences(&self) -> impl Iterator<Item = &SequenceRecord> {
        self.row_visibility
            .visible_to_absolute()
            .iter()
            .map(|&sequence_id| &self.data.sequences[sequence_id])
    }

    /// Yields visible pinned sequences in pin order.
    pub fn pinned_sequences(&self) -> impl Iterator<Item = &SequenceRecord> {
        self.pinned_sequence_ids
            .iter()
            .copied()
            .filter(|&sequence_id| !self.row_visibility.is_hidden(sequence_id))
            .map(|sequence_id| &self.data.sequences[sequence_id])
    }

    #[must_use]
    pub fn visible_pinned_count(&self) -> usize {
        self.pinned_sequence_ids
            .iter()
            .copied()
            .filter(|&sequence_id| !self.row_visibility.is_hidden(sequence_id))
            .count()
    }
    /// Yields visible unpinned sequences in original alignment order.
    pub fn visible_unpinned_sequences(&self) -> impl Iterator<Item = &SequenceRecord> {
        self.row_visibility
            .visible_to_absolute()
            .iter()
            .skip(self.visible_pinned_count())
            .map(|&sequence_id| &self.data.sequences[sequence_id])
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

    /// Updates `row_visibility` hidden flags
    ///
    /// Reference rows are always hidden. Pinned rows always bypass filter exclusion.
    /// Unpinned rows are hidden when they do not match the active filter.
    fn apply_hide_flags(&mut self) {
        let reference_index = self.reference_sequence_id;
        let filter = self.filter.regex.as_ref();
        let sequences = &self.data.sequences;
        let pinned_sequence_ids = &self.pinned_sequence_ids;

        self.row_visibility.set_hidden(|sequence_id| {
            let sequence = &sequences[sequence_id];
            let is_reference = reference_index == Some(sequence_id);
            let is_pinned = pinned_sequence_ids.contains(&sequence_id);
            let excluded_by_filter = !is_pinned
                && filter.is_some_and(|regex| !regex.is_match(sequence.alignment.id.as_ref()));
            is_reference || excluded_by_filter
        });
    }

    /// Rebuilds row visibility in display order.
    ///
    /// Pinned rows are first in pin order, then visible remaining rows.
    fn rebuild_row_visibility(&mut self) {
        let visible_pinned: Vec<usize> = self
            .pinned_sequence_ids
            .iter()
            .copied()
            .filter(|&id| !self.row_visibility.is_hidden(id))
            .collect();

        let visible_unpinned = self
            .data
            .sequences
            .iter()
            .map(|s| s.sequence_id)
            .filter(|&id| !self.row_visibility.is_hidden(id) && !self.is_sequence_pinned(id));

        let ordered: Vec<usize> = visible_pinned
            .iter()
            .copied()
            .chain(visible_unpinned)
            .collect();

        self.row_visibility.set_visible_order(&ordered);
    }

    /// Recomputes viewport params
    fn refresh_viewport(&mut self) {
        self.apply_hide_flags();
        self.rebuild_row_visibility();
        self.viewport.max_size.rows = self.row_visibility.visible_count();
        self.viewport.max_size.cols = self.data.sequence_length;
        self.viewport.max_size.name_width = self.data.max_sequence_id_len;
        self.viewport.clamp_offsets();
    }

    /// Applies or clears the ID filter pattern and refreshes derived viewport state.
    fn apply_filter(&mut self, filter: Option<(String, Regex)>) {
        // none = clear filter
        let Some((pattern, regex)) = filter else {
            self.filter.text.clear();
            self.filter.regex = None;
            self.refresh_viewport();
            return;
        };

        // pattern is empty = also clear filter
        // this is for UX reasons, so users can clear the filter by deleting the pattern as well
        // as using `clear-filter` command
        if pattern.is_empty() {
            self.filter.text.clear();
            self.filter.regex = None;
        } else {
            self.filter.text = pattern;
            self.filter.regex = Some(regex);
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

        let visible_sequences: Vec<SequenceRecord> =
            self.all_visible_sequences().cloned().collect();
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
        core.handle_alignments_loaded(Ok(alignments));
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
            .all_visible_sequences()
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
            .all_visible_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();
        assert_eq!(visible_ids, vec![2, 0, 1]);
    }

    #[test]
    fn pinned_sequences_excludes_reference_row() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);

        core.pinned_sequence_ids.push(1);
        core.reference_sequence_id = Some(1);
        core.refresh_viewport();

        let pinned_ids: Vec<_> = core
            .pinned_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();

        assert!(pinned_ids.is_empty());
        assert_eq!(core.visible_pinned_count(), 0);
    }

    #[test]
    fn jump_to_sequence_uses_offset() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c", "seq-d"]);

        core.apply_action(CoreAction::PinSequence(2));
        core.apply_action(CoreAction::PinSequence(0));
        core.apply_action(CoreAction::JumpToSequence(3));

        assert_eq!(core.viewport.offsets.rows, 1);
    }

    #[test]
    fn jump_to_sequence_ignores_hidden() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);

        core.apply_action(CoreAction::SetReference(1));
        assert!(core
            .all_visible_sequences()
            .all(|sequence| sequence.sequence_id != 1));

        core.viewport.offsets.rows = 1;
        core.apply_action(CoreAction::JumpToSequence(1));

        assert_eq!(core.viewport.offsets.rows, 1);
    }
    #[test]
    fn setting_reference_removes_existing_pin() {
        let mut core = test_core_with_ids(&["seq-a", "seq-b", "seq-c"]);

        core.apply_action(CoreAction::PinSequence(1));
        core.apply_action(CoreAction::SetReference(1));

        assert!(!core.is_sequence_pinned(1));
        let visible_ids: Vec<_> = core
            .all_visible_sequences()
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
            .all_visible_sequences()
            .map(|sequence| sequence.sequence_id)
            .collect();
        assert_eq!(visible_ids, vec![1]);
    }
}
