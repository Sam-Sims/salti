use std::{fmt, ops::Range, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatsView {
    Raw,
    Translated(libmsa::ReadingFrame),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsContext {
    pub view: StatsView,
    pub range: Range<usize>,
    pub total_columns: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DiffMode {
    #[default]
    Off,
    Reference,
    Consensus,
}

impl DiffMode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Reference => "reference",
            Self::Consensus => "consensus",
        }
    }

    pub const fn all() -> [Self; 3] {
        [Self::Off, Self::Reference, Self::Consensus]
    }
}

impl fmt::Display for DiffMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for DiffMode {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::all()
            .into_iter()
            .find(|mode| mode.name() == value)
            .ok_or_else(|| anyhow::format_err!("invalid diff mode: {value}"))
    }
}

#[derive(Debug, Default, Clone)]
pub struct RowPresentationState {
    pinned: Vec<usize>,
    reference: Option<usize>,
}

impl RowPresentationState {
    pub fn pinned(&self) -> &[usize] {
        &self.pinned
    }

    pub fn reference(&self) -> Option<usize> {
        self.reference
    }

    pub fn is_pinned(&self, abs_row: usize) -> bool {
        self.pinned.contains(&abs_row)
    }

    pub fn is_reference(&self, abs_row: usize) -> bool {
        self.reference == Some(abs_row)
    }

    pub fn excluded_rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.pinned.iter().copied().chain(self.reference)
    }

    pub fn pin(&mut self, abs_row: usize, row_count: usize) -> Result<(), libmsa::AlignmentError> {
        validate_row_id(abs_row, row_count)?;
        if self.reference == Some(abs_row) || self.pinned.contains(&abs_row) {
            return Err(libmsa::AlignmentError::DuplicateRowIndex { index: abs_row });
        }
        self.pinned.push(abs_row);
        Ok(())
    }

    pub fn unpin(
        &mut self,
        abs_row: usize,
        row_count: usize,
    ) -> Result<(), libmsa::AlignmentError> {
        validate_row_id(abs_row, row_count)?;
        self.pinned.retain(|&pinned_row| pinned_row != abs_row);
        Ok(())
    }

    pub fn set_reference(
        &mut self,
        abs_row: usize,
        row_count: usize,
    ) -> Result<(), libmsa::AlignmentError> {
        validate_row_id(abs_row, row_count)?;
        self.pinned.retain(|&pinned_row| pinned_row != abs_row);
        self.reference = Some(abs_row);
        Ok(())
    }

    pub fn clear_reference(&mut self) {
        self.reference = None;
    }
}

#[derive(Debug, Default, Clone)]
pub struct FilterState {
    pattern: Option<String>,
    max_gap_fraction: Option<f32>,
}

impl FilterState {
    pub fn pattern(&self) -> Option<&str> {
        self.pattern.as_deref()
    }

    pub fn max_gap_fraction(&self) -> Option<f32> {
        self.max_gap_fraction
    }

    pub fn has_column_filter(&self) -> bool {
        self.max_gap_fraction.is_some()
    }

    pub fn is_active(&self) -> bool {
        self.pattern.is_some() || self.max_gap_fraction.is_some()
    }
}

#[derive(Debug)]
pub struct AlignmentModel {
    base: libmsa::Alignment,
    view: libmsa::Alignment,
    rows: RowPresentationState,
    filter: FilterState,
    translation_enabled: bool,
    translation_frame: libmsa::ReadingFrame,
    pub diff_mode: DiffMode,
    pub consensus_method: libmsa::ConsensusMethod,
}

impl AlignmentModel {
    pub fn new(base: libmsa::Alignment) -> Result<Self, libmsa::AlignmentError> {
        if base.is_filtered() {
            return Err(libmsa::AlignmentError::UnsupportedOperation {
                operation: "create alignment model from filtered alignment",
                kind: base.active_type(),
            });
        }

        Ok(Self {
            view: base.clone(),
            base,
            rows: RowPresentationState::default(),
            filter: FilterState::default(),
            translation_enabled: false,
            translation_frame: libmsa::ReadingFrame::Frame1,
            diff_mode: DiffMode::default(),
            consensus_method: libmsa::ConsensusMethod::default(),
        })
    }

    pub fn base(&self) -> &libmsa::Alignment {
        &self.base
    }

    pub fn view(&self) -> &libmsa::Alignment {
        &self.view
    }

    pub fn rows(&self) -> &RowPresentationState {
        &self.rows
    }

    pub fn filter(&self) -> &FilterState {
        &self.filter
    }

    pub fn translation(&self) -> Option<libmsa::ReadingFrame> {
        self.translation_enabled.then_some(self.translation_frame)
    }

    #[cfg(test)]
    pub fn translation_frame(&self) -> libmsa::ReadingFrame {
        self.translation_frame
    }

    pub fn pin(&mut self, abs_row: usize) -> Result<(), libmsa::AlignmentError> {
        self.rows.pin(abs_row, self.base_row_count())?;
        self.derive_view_from_intent()
    }

    pub fn unpin(&mut self, abs_row: usize) -> Result<(), libmsa::AlignmentError> {
        self.rows.unpin(abs_row, self.base_row_count())?;
        self.derive_view_from_intent()
    }

    pub fn set_reference(&mut self, abs_row: usize) -> Result<(), libmsa::AlignmentError> {
        self.rows.set_reference(abs_row, self.base_row_count())?;
        self.derive_view_from_intent()
    }

    pub fn clear_reference(&mut self) -> Result<(), libmsa::AlignmentError> {
        self.rows.clear_reference();
        self.derive_view_from_intent()
    }

    pub fn set_filter(&mut self, pattern: String) -> Result<(), libmsa::AlignmentError> {
        let next_pattern = if pattern.is_empty() {
            None
        } else {
            Some(pattern)
        };
        let previous = std::mem::replace(&mut self.filter.pattern, next_pattern);
        if let Err(error) = self.derive_view_from_intent() {
            self.filter.pattern = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn set_gap_filter(
        &mut self,
        max_gap_fraction: Option<f32>,
    ) -> Result<(), libmsa::AlignmentError> {
        let previous = self.filter.max_gap_fraction;
        self.filter.max_gap_fraction = max_gap_fraction;
        if let Err(error) = self.derive_view_from_intent() {
            self.filter.max_gap_fraction = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn clear_filter(&mut self) -> Result<(), libmsa::AlignmentError> {
        self.filter.pattern = None;
        self.filter.max_gap_fraction = None;
        self.derive_view_from_intent()
    }

    pub fn set_active_kind(
        &mut self,
        kind: libmsa::AlignmentType,
    ) -> Result<(), libmsa::AlignmentError> {
        self.base.set_override_type(kind);
        if kind != libmsa::AlignmentType::Dna {
            self.translation_enabled = false;
        }
        self.derive_view_from_intent()
    }

    pub fn set_translation(
        &mut self,
        frame: Option<libmsa::ReadingFrame>,
    ) -> Result<(), libmsa::AlignmentError> {
        let Some(frame) = frame else {
            self.translation_enabled = false;
            return Ok(());
        };

        if self.base.active_type() != libmsa::AlignmentType::Dna {
            return Err(libmsa::AlignmentError::UnsupportedOperation {
                operation: "set translation",
                kind: self.base.active_type(),
            });
        }

        self.view.translated(frame)?;
        self.translation_enabled = true;
        self.translation_frame = frame;
        Ok(())
    }

    pub fn set_translation_frame(
        &mut self,
        frame: libmsa::ReadingFrame,
    ) -> Result<(), libmsa::AlignmentError> {
        if self.base.active_type() != libmsa::AlignmentType::Dna {
            return Err(libmsa::AlignmentError::UnsupportedOperation {
                operation: "set translation frame",
                kind: self.base.active_type(),
            });
        }

        self.view.translated(frame)?;
        self.translation_frame = frame;
        Ok(())
    }

    pub fn toggle_translation_view(&mut self) -> Result<(), libmsa::AlignmentError> {
        if self.translation_enabled {
            self.translation_enabled = false;
            return Ok(());
        }
        self.set_translation(Some(self.translation_frame))
    }

    pub fn translated_view(&self) -> Option<libmsa::TranslatedAlignment<'_>> {
        let frame = self.translation()?;
        self.view.translated(frame).ok()
    }

    pub fn stats_context(&self, visible_col_range: Range<usize>) -> Option<StatsContext> {
        if let Some(frame) = self.translation() {
            let nucleotide_len = self.view().column_count();
            let total_columns = complete_protein_len(frame, nucleotide_len);
            if total_columns == 0 {
                return None;
            }
            let range = visible_protein_range(&visible_col_range, frame, nucleotide_len)?;
            return Some(StatsContext {
                view: StatsView::Translated(frame),
                range,
                total_columns,
            });
        }

        let total_columns = self.view().column_count();
        if total_columns == 0 || visible_col_range.is_empty() {
            return None;
        }

        Some(StatsContext {
            view: StatsView::Raw,
            range: visible_col_range,
            total_columns,
        })
    }

    pub fn jump_to_sequence(&self, abs_row: usize) -> Option<String> {
        if self.view().relative_row_id(abs_row).is_some() {
            return None;
        }

        Some(if self.rows().is_pinned(abs_row) {
            "Sequence is pinned".to_string()
        } else if self.rows().is_reference(abs_row) {
            "Sequence is set as reference".to_string()
        } else {
            "Sequence is not visible in the current view".to_string()
        })
    }

    fn base_row_count(&self) -> usize {
        self.base.row_count()
    }

    fn derive_view_from_intent(&mut self) -> Result<(), libmsa::AlignmentError> {
        let mut builder = self.base.filter()?;
        builder = builder.without_rows(self.rows.excluded_rows());
        if let Some(pattern) = self.filter.pattern() {
            builder = builder.with_row_regex(pattern);
        }
        if let Some(max_gap_fraction) = self.filter.max_gap_fraction() {
            builder = builder.with_max_gap_fraction(max_gap_fraction);
        }
        self.view = builder.apply()?;
        Ok(())
    }
}

fn validate_row_id(abs_row: usize, row_count: usize) -> Result<(), libmsa::AlignmentError> {
    if abs_row < row_count {
        return Ok(());
    }
    Err(libmsa::AlignmentError::RowOutOfBounds {
        index: abs_row,
        row_count,
    })
}

const fn complete_protein_len(frame: libmsa::ReadingFrame, nucleotide_len: usize) -> usize {
    nucleotide_len.saturating_sub(frame.offset()) / 3
}

fn visible_protein_range(
    visible_nucleotide_range: &Range<usize>,
    frame: libmsa::ReadingFrame,
    nucleotide_len: usize,
) -> Option<Range<usize>> {
    let last_visible_col = visible_nucleotide_range.end.checked_sub(1)?;
    if last_visible_col < frame.offset() {
        return None;
    }

    let protein_len = complete_protein_len(frame, nucleotide_len);
    if protein_len == 0 {
        return None;
    }

    let start = visible_nucleotide_range
        .start
        .saturating_sub(frame.offset())
        / 3;
    let end = ((last_visible_col - frame.offset()) / 3 + 1).min(protein_len);

    (start < end).then_some(start..end)
}

#[cfg(test)]
mod tests {
    use super::{AlignmentModel, DiffMode, RowPresentationState, StatsContext, StatsView};

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn alignment_model(sequences: Vec<libmsa::RawSequence>) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(sequences).expect("alignment should be valid");
        AlignmentModel::new(alignment).expect("alignment model should build")
    }

    #[test]
    fn row_presentation_state_rejects_duplicate_pins() {
        let mut state = RowPresentationState::default();
        state.pin(1, 3).unwrap();

        let error = state.pin(1, 3).unwrap_err();

        assert_eq!(
            error,
            libmsa::AlignmentError::DuplicateRowIndex { index: 1 }
        );
    }

    #[test]
    fn row_presentation_state_rejects_pinning_the_reference() {
        let mut state = RowPresentationState::default();
        state.set_reference(1, 3).unwrap();

        let error = state.pin(1, 3).unwrap_err();

        assert_eq!(
            error,
            libmsa::AlignmentError::DuplicateRowIndex { index: 1 }
        );
    }

    #[test]
    fn row_presentation_state_sets_reference() {
        let mut state = RowPresentationState::default();

        state.set_reference(1, 3).unwrap();

        assert_eq!(state.reference(), Some(1));
    }

    #[test]
    fn row_presentation_state_removes_row_when_reference_set() {
        let mut state = RowPresentationState::default();
        state.pin(1, 3).unwrap();

        state.set_reference(1, 3).unwrap();

        assert!(!state.is_pinned(1));
    }

    #[test]
    fn row_presentation_state_excluded_rows() {
        let mut state = RowPresentationState::default();
        state.pin(1, 4).unwrap();
        state.pin(3, 4).unwrap();
        state.set_reference(2, 4).unwrap();

        let excluded_rows = state.excluded_rows().collect::<Vec<_>>();

        assert_eq!(excluded_rows, vec![1, 3, 2]);
    }

    #[test]
    fn row_presentation_state_rejects_out_of_bounds() {
        let mut state = RowPresentationState::default();

        let pin_error = state.pin(3, 3).unwrap_err();
        let unpin_error = state.unpin(3, 3).unwrap_err();
        let reference_error = state.set_reference(3, 3).unwrap_err();

        assert_eq!(
            pin_error,
            libmsa::AlignmentError::RowOutOfBounds {
                index: 3,
                row_count: 3,
            }
        );
        assert_eq!(
            unpin_error,
            libmsa::AlignmentError::RowOutOfBounds {
                index: 3,
                row_count: 3,
            }
        );
        assert_eq!(
            reference_error,
            libmsa::AlignmentError::RowOutOfBounds {
                index: 3,
                row_count: 3,
            }
        );
    }

    #[test]
    fn alignment_model_new_clones_base_into_view() {
        let model = alignment_model(vec![raw("row1", b"ACGT"), raw("row2", b"TGCA")]);

        assert_eq!(model.base().row_count(), 2);
        assert_eq!(model.view().row_count(), 2);
        assert_eq!(model.diff_mode, DiffMode::Off);
        assert_eq!(model.consensus_method, libmsa::ConsensusMethod::default());
    }

    #[test]
    fn alignment_model_new_rejects_filtered_alignment() {
        let alignment = libmsa::Alignment::new(vec![raw("row1", b"ACGT"), raw("row2", b"ACGT")])
            .expect("alignment should be valid")
            .filter()
            .expect("filter builder should build")
            .with_row_regex("row1")
            .apply()
            .expect("filtered alignment should build");

        let error = AlignmentModel::new(alignment).unwrap_err();

        assert_eq!(
            error,
            libmsa::AlignmentError::UnsupportedOperation {
                operation: "create alignment model from filtered alignment",
                kind: libmsa::AlignmentType::Dna,
            }
        );
    }

    #[test]
    fn pin_hides_row_from_view() {
        let mut model = alignment_model(vec![
            raw("row1", b"ACGT"),
            raw("row2", b"ACGT"),
            raw("row3", b"ACGT"),
        ]);

        model.pin(1).unwrap();

        assert_eq!(model.rows().pinned(), &[1]);
        assert_eq!(model.view().row_count(), 2);
        assert_eq!(model.view().relative_row_id(1), None);
    }

    #[test]
    fn unpin_restores_row_to_view() {
        let mut model = alignment_model(vec![
            raw("row1", b"ACGT"),
            raw("row2", b"ACGT"),
            raw("row3", b"ACGT"),
        ]);
        model.pin(1).unwrap();

        model.unpin(1).unwrap();

        assert!(model.rows().pinned().is_empty());
        assert_eq!(model.view().row_count(), 3);
        assert_eq!(model.view().relative_row_id(1), Some(1));
    }

    #[test]
    fn set_reference_hides_row_from_view() {
        let mut model = alignment_model(vec![
            raw("row1", b"ACGT"),
            raw("row2", b"ACGT"),
            raw("row3", b"ACGT"),
        ]);

        model.set_reference(1).unwrap();

        assert_eq!(model.rows().reference(), Some(1));
        assert_eq!(model.view().row_count(), 2);
        assert_eq!(model.view().relative_row_id(1), None);
    }

    #[test]
    fn clear_reference_restores_row_to_view() {
        let mut model = alignment_model(vec![raw("row1", b"ACGT"), raw("row2", b"ACGT")]);
        model.set_reference(1).unwrap();

        model.clear_reference().unwrap();

        assert_eq!(model.rows().reference(), None);
        assert_eq!(model.view().relative_row_id(1), Some(1));
    }

    #[test]
    fn set_filter_applies_row_pattern() {
        let mut model = alignment_model(vec![
            raw("alpha", b"ACGT"),
            raw("beta", b"ACGT"),
            raw("gamma", b"ACGT"),
        ]);

        model.set_filter("alpha|beta".to_string()).unwrap();

        assert_eq!(model.filter().pattern(), Some("alpha|beta"));
        assert_eq!(model.view().row_count(), 2);
    }

    #[test]
    fn set_filter_treats_empty_string_as_clear() {
        let mut model = alignment_model(vec![
            raw("alpha", b"ACGT"),
            raw("beta", b"ACGT"),
            raw("gamma", b"ACGT"),
        ]);
        model.set_filter("alpha|beta".to_string()).unwrap();

        model.set_filter(String::new()).unwrap();

        assert_eq!(model.filter().pattern(), None);
        assert_eq!(model.view().row_count(), 3);
    }

    #[test]
    fn set_filter_restores_previous_pattern_on_error() {
        let mut model = alignment_model(vec![raw("alpha", b"ACGT"), raw("beta", b"ACGT")]);
        model.set_filter("alpha".to_string()).unwrap();

        let error = model.set_filter("(".to_string()).unwrap_err();

        assert_eq!(model.filter().pattern(), Some("alpha"));
        assert!(matches!(error, libmsa::AlignmentError::InvalidRegex { .. }));
    }

    #[test]
    fn set_gap_filter_applies_column_filter() {
        let mut model = alignment_model(vec![
            raw("alpha", b"A--T"),
            raw("beta", b"A--T"),
            raw("gamma", b"ACGT"),
        ]);

        model.set_gap_filter(Some(0.0)).unwrap();

        assert_eq!(model.filter().max_gap_fraction(), Some(0.0));
        assert_eq!(model.view().column_count(), 2);
    }

    #[test]
    fn clear_filter_removes_both_filters() {
        let mut model = alignment_model(vec![
            raw("alpha", b"A--T"),
            raw("beta", b"A--T"),
            raw("gamma", b"ACGT"),
        ]);
        model.set_filter("alpha|beta".to_string()).unwrap();
        model.set_gap_filter(Some(0.0)).unwrap();

        model.clear_filter().unwrap();

        assert_eq!(model.filter().pattern(), None);
        assert_eq!(model.filter().max_gap_fraction(), None);
        assert_eq!(model.view().row_count(), 3);
        assert_eq!(model.view().column_count(), 4);
    }

    #[test]
    fn set_active_kind_disables_translation_when_leaving_dna() {
        let mut model = alignment_model(vec![raw("dna", b"ATGAAATTT")]);
        model
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap();

        model
            .set_active_kind(libmsa::AlignmentType::Protein)
            .unwrap();

        assert_eq!(model.translation(), None);
        assert_eq!(model.translation_frame(), libmsa::ReadingFrame::Frame1);
    }

    #[test]
    fn set_translation_enables_translation_for_dna() {
        let mut model = alignment_model(vec![raw("dna", b"ATGAAATTT")]);

        model
            .set_translation(Some(libmsa::ReadingFrame::Frame2))
            .unwrap();

        assert_eq!(model.translation(), Some(libmsa::ReadingFrame::Frame2));
        assert_eq!(model.translation_frame(), libmsa::ReadingFrame::Frame2);
    }

    #[test]
    fn set_translation_rejects_non_dna_alignments() {
        let mut model = alignment_model(vec![raw("aa", b"MKF")]);
        model
            .set_active_kind(libmsa::AlignmentType::Protein)
            .unwrap();

        let error = model
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap_err();

        assert_eq!(
            error,
            libmsa::AlignmentError::UnsupportedOperation {
                operation: "set translation",
                kind: libmsa::AlignmentType::Protein,
            }
        );
    }

    #[test]
    fn set_translation_frame_updates_stored_frame() {
        let mut model = alignment_model(vec![raw("dna", b"ATGAAATTT")]);

        model
            .set_translation_frame(libmsa::ReadingFrame::Frame3)
            .unwrap();

        assert_eq!(model.translation(), None);
        assert_eq!(model.translation_frame(), libmsa::ReadingFrame::Frame3);
    }

    #[test]
    fn set_translation_frame_rejects_non_dna_alignments() {
        let mut model = alignment_model(vec![raw("aa", b"MKF")]);
        model
            .set_active_kind(libmsa::AlignmentType::Protein)
            .unwrap();

        let error = model
            .set_translation_frame(libmsa::ReadingFrame::Frame1)
            .unwrap_err();

        assert_eq!(
            error,
            libmsa::AlignmentError::UnsupportedOperation {
                operation: "set translation frame",
                kind: libmsa::AlignmentType::Protein,
            }
        );
    }

    #[test]
    fn stats_context_returns_raw_range_unchanged() {
        let model = alignment_model(vec![raw("row1", b"ACGT"), raw("row2", b"ACGT")]);

        assert_eq!(
            model.stats_context(1..3),
            Some(StatsContext {
                view: StatsView::Raw,
                range: 1..3,
                total_columns: 4,
            })
        );
    }

    #[test]
    fn stats_context_maps_translated_range_to_protein_columns() {
        let mut model = alignment_model(vec![raw("row1", b"ATGAAATTT")]);
        model
            .set_translation(Some(libmsa::ReadingFrame::Frame2))
            .unwrap();

        assert_eq!(
            model.stats_context(0..9),
            Some(StatsContext {
                view: StatsView::Translated(libmsa::ReadingFrame::Frame2),
                range: 0..2,
                total_columns: 2,
            })
        );
    }

    #[test]
    fn stats_context_returns_none_when_no_columns_can_be_computed() {
        let model = alignment_model(vec![raw("row1", b"ACGT")]);

        assert_eq!(model.stats_context(0..0), None);

        let mut translated = alignment_model(vec![raw("row1", b"AT")]);
        translated
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap();
        assert_eq!(translated.stats_context(0..2), None);
    }

    #[test]
    fn jump_to_sequence_reports_why_hidden_rows_are_not_visible() {
        let mut model = alignment_model(vec![
            raw("row1", b"ACGT"),
            raw("row2", b"ACGT"),
            raw("row3", b"ACGT"),
            raw("row4", b"ACGT"),
        ]);
        model.pin(0).unwrap();
        model.set_reference(1).unwrap();
        model.set_filter("row4".to_string()).unwrap();

        assert_eq!(
            model.jump_to_sequence(0),
            Some("Sequence is pinned".to_string())
        );
        assert_eq!(
            model.jump_to_sequence(1),
            Some("Sequence is set as reference".to_string())
        );
        assert_eq!(
            model.jump_to_sequence(2),
            Some("Sequence is not visible in the current view".to_string())
        );
    }
}
