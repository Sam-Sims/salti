use std::{fmt, ops::Range, str::FromStr};

use crate::core::codon::{TranslationOverlay, complete_protein_len, visible_protein_range};

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
    min_constant_fraction: Option<f32>,
}

impl FilterState {
    pub fn pattern(&self) -> Option<&str> {
        self.pattern.as_deref()
    }

    pub fn max_gap_fraction(&self) -> Option<f32> {
        self.max_gap_fraction
    }

    pub fn min_constant_fraction(&self) -> Option<f32> {
        self.min_constant_fraction
    }

    pub fn has_column_filter(&self) -> bool {
        self.max_gap_fraction.is_some() || self.min_constant_fraction.is_some()
    }

    pub fn is_active(&self) -> bool {
        self.pattern.is_some()
            || self.max_gap_fraction.is_some()
            || self.min_constant_fraction.is_some()
    }
}

#[derive(Debug, Clone)]
struct Stash {
    base: libmsa::Alignment,
}

#[derive(Debug, Clone)]
enum TranslationMode {
    Off,
    Overlay,
    ReloadedProtein { stash: Stash },
}

#[derive(Debug)]
pub struct AlignmentModel {
    base: libmsa::Alignment,
    view: libmsa::Alignment,
    rows: RowPresentationState,
    filter: FilterState,
    translation_mode: TranslationMode,
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
            translation_mode: TranslationMode::Off,
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
        match &self.translation_mode {
            TranslationMode::Overlay => Some(self.translation_frame),
            TranslationMode::Off | TranslationMode::ReloadedProtein { .. } => None,
        }
    }

    pub fn is_reloaded_as_protein(&self) -> bool {
        matches!(
            &self.translation_mode,
            TranslationMode::ReloadedProtein { .. }
        )
    }

    pub fn translation_frame(&self) -> libmsa::ReadingFrame {
        self.translation_frame
    }

    pub fn pin(&mut self, abs_row: usize) -> Result<(), libmsa::AlignmentError> {
        self.rows.pin(abs_row, self.base_row_count())?;
        self.update_current_view()
    }

    pub fn unpin(&mut self, abs_row: usize) -> Result<(), libmsa::AlignmentError> {
        self.rows.unpin(abs_row, self.base_row_count())?;
        self.update_current_view()
    }

    pub fn set_reference(&mut self, abs_row: usize) -> Result<(), libmsa::AlignmentError> {
        self.rows.set_reference(abs_row, self.base_row_count())?;
        self.update_current_view()
    }

    pub fn clear_reference(&mut self) -> Result<(), libmsa::AlignmentError> {
        self.rows.clear_reference();
        self.update_current_view()
    }

    pub fn set_filter(&mut self, pattern: String) -> Result<(), libmsa::AlignmentError> {
        let next_pattern = if pattern.is_empty() {
            None
        } else {
            Some(pattern)
        };
        let previous = std::mem::replace(&mut self.filter.pattern, next_pattern);
        if let Err(error) = self.update_current_view() {
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
        if let Err(error) = self.update_current_view() {
            self.filter.max_gap_fraction = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn set_constant_filter(
        &mut self,
        min_constant_fraction: Option<f32>,
    ) -> Result<(), libmsa::AlignmentError> {
        let previous = self.filter.min_constant_fraction;
        self.filter.min_constant_fraction = min_constant_fraction;
        if let Err(error) = self.update_current_view() {
            self.filter.min_constant_fraction = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn clear_filter(&mut self) -> Result<(), libmsa::AlignmentError> {
        self.filter.pattern = None;
        self.filter.max_gap_fraction = None;
        self.filter.min_constant_fraction = None;
        self.update_current_view()
    }

    pub fn set_active_kind(
        &mut self,
        kind: libmsa::AlignmentType,
    ) -> Result<(), libmsa::AlignmentError> {
        self.base.set_override_type(kind);
        if kind != libmsa::AlignmentType::Dna
            && matches!(&self.translation_mode, TranslationMode::Overlay)
        {
            self.translation_mode = TranslationMode::Off;
        }
        self.update_current_view()
    }

    pub fn set_translation(
        &mut self,
        frame: Option<libmsa::ReadingFrame>,
    ) -> Result<(), libmsa::AlignmentError> {
        let Some(frame) = frame else {
            if !matches!(
                &self.translation_mode,
                TranslationMode::ReloadedProtein { .. }
            ) {
                self.translation_mode = TranslationMode::Off;
            }
            return Ok(());
        };

        if self.base.active_type() != libmsa::AlignmentType::Dna {
            return Err(libmsa::AlignmentError::UnsupportedOperation {
                operation: "set translation",
                kind: self.base.active_type(),
            });
        }

        self.view.translated(frame)?;
        self.translation_mode = TranslationMode::Overlay;
        self.translation_frame = frame;
        Ok(())
    }

    pub fn set_translation_frame(
        &mut self,
        frame: libmsa::ReadingFrame,
    ) -> Result<(), libmsa::AlignmentError> {
        let protein = match &self.translation_mode {
            TranslationMode::ReloadedProtein { stash: dna_stash } => {
                Some(dna_stash.base.translated(frame)?.to_alignment()?)
            }
            TranslationMode::Off | TranslationMode::Overlay => None,
        };

        if let Some(protein) = protein {
            self.translation_frame = frame;
            self.base = protein;
            return self.update_current_view();
        }

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
        if matches!(&self.translation_mode, TranslationMode::Off) {
            return self.set_translation(Some(self.translation_frame));
        }

        if matches!(&self.translation_mode, TranslationMode::Overlay) {
            self.translation_mode = TranslationMode::Off;
            return Ok(());
        }

        Err(libmsa::AlignmentError::UnsupportedOperation {
            operation: "set translation",
            kind: self.base.active_type(),
        })
    }

    pub fn toggle_reload_as_protein(
        &mut self,
        frame: Option<libmsa::ReadingFrame>,
    ) -> Result<(), libmsa::AlignmentError> {
        if let Some(frame) = frame {
            self.translation_frame = frame;
        }

        if matches!(
            &self.translation_mode,
            TranslationMode::ReloadedProtein { .. }
        ) {
            let translation_mode =
                std::mem::replace(&mut self.translation_mode, TranslationMode::Off);
            let dna_stash = match translation_mode {
                TranslationMode::ReloadedProtein { stash: dna_stash } => dna_stash,
                TranslationMode::Off | TranslationMode::Overlay => unreachable!(),
            };

            self.base = dna_stash.base;
            self.translation_mode = TranslationMode::Off;
            return self.update_current_view();
        }

        let protein = self
            .base
            .translated(self.translation_frame)?
            .to_alignment()?;
        let dna_stash = Stash {
            base: self.base.clone(),
        };
        self.base = protein;
        self.translation_mode = TranslationMode::ReloadedProtein { stash: dna_stash };
        self.update_current_view()
    }

    pub fn translated_view(&self) -> Option<libmsa::TranslatedAlignment<'_>> {
        match &self.translation_mode {
            TranslationMode::Overlay => self.view.translated(self.translation_frame).ok(),
            TranslationMode::Off | TranslationMode::ReloadedProtein { .. } => None,
        }
    }

    pub(crate) fn translation_overlay(&self) -> Option<TranslationOverlay> {
        match &self.translation_mode {
            TranslationMode::Overlay => Some(TranslationOverlay {
                frame: self.translation_frame,
                nucleotide_len: self.view().column_count(),
            }),
            TranslationMode::Off | TranslationMode::ReloadedProtein { .. } => None,
        }
    }

    pub fn stats_context(&self, visible_col_range: Range<usize>) -> Option<StatsContext> {
        match &self.translation_mode {
            TranslationMode::Overlay => {
                let frame = self.translation_frame;
                let nucleotide_len = self.view().column_count();
                let total_columns = complete_protein_len(frame, nucleotide_len);
                if total_columns == 0 {
                    return None;
                }
                let range = visible_protein_range(&visible_col_range, frame, nucleotide_len)?;
                Some(StatsContext {
                    view: StatsView::Translated(frame),
                    range,
                    total_columns,
                })
            }
            TranslationMode::Off | TranslationMode::ReloadedProtein { .. } => {
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
        }
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

    fn update_current_view(&mut self) -> Result<(), libmsa::AlignmentError> {
        let mut builder = self.base.filter()?;
        builder = builder.without_rows(self.rows.excluded_rows());
        if let Some(pattern) = self.filter.pattern() {
            builder = builder.with_row_regex(pattern);
        }
        if let Some(max_gap_fraction) = self.filter.max_gap_fraction() {
            builder = builder.with_max_gap_fraction(max_gap_fraction);
        }
        if let Some(min_constant_fraction) = self.filter.min_constant_fraction() {
            builder = builder.with_min_constant_fraction(min_constant_fraction);
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
        let alignment = libmsa::Alignment::new(sequences).unwrap();
        AlignmentModel::new(alignment).unwrap()
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
    fn row_presentation_state_rejects_pinning_reference() {
        let mut state = RowPresentationState::default();
        state.set_reference(1, 3).unwrap();

        let error = state.pin(1, 3).unwrap_err();

        assert_eq!(
            error,
            libmsa::AlignmentError::DuplicateRowIndex { index: 1 }
        );
    }

    #[test]
    fn row_presentation_state_sets_reference_and_unpins_row() {
        let mut state = RowPresentationState::default();
        state.pin(1, 3).unwrap();

        state.set_reference(1, 3).unwrap();

        assert_eq!(state.reference(), Some(1));
        assert!(!state.is_pinned(1));
    }

    #[test]
    fn row_presentation_state_lists_pinned_rows_before_reference() {
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
    fn alignment_model_new_sets_view_and_defaults() {
        let model = alignment_model(vec![raw("seq1", b"CATC"), raw("seq2", b"ATAC")]);

        assert_eq!(model.base().row_count(), 2);
        assert_eq!(model.view().row_count(), 2);
        assert_eq!(model.diff_mode, DiffMode::Off);
        assert_eq!(model.consensus_method, libmsa::ConsensusMethod::default());
    }

    #[test]
    fn alignment_model_new_rejects_filtered_alignment() {
        let alignment = libmsa::Alignment::new(vec![raw("seq1", b"CATC"), raw("seq2", b"CATC")])
            .unwrap()
            .filter()
            .unwrap()
            .with_row_regex("seq1")
            .apply()
            .unwrap();

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
            raw("seq1", b"CATC"),
            raw("seq2", b"CATC"),
            raw("seq3", b"CATC"),
        ]);

        model.pin(1).unwrap();

        assert_eq!(model.rows().pinned(), &[1]);
        assert_eq!(model.view().row_count(), 2);
        assert_eq!(model.view().relative_row_id(1), None);
    }

    #[test]
    fn unpin_restores_row_to_view() {
        let mut model = alignment_model(vec![
            raw("seq1", b"CATC"),
            raw("seq2", b"CATC"),
            raw("seq3", b"CATC"),
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
            raw("seq1", b"CATC"),
            raw("seq2", b"CATC"),
            raw("seq3", b"CATC"),
        ]);

        model.set_reference(1).unwrap();

        assert_eq!(model.rows().reference(), Some(1));
        assert_eq!(model.view().row_count(), 2);
        assert_eq!(model.view().relative_row_id(1), None);
    }

    #[test]
    fn clear_reference_restores_row_to_view() {
        let mut model = alignment_model(vec![raw("seq1", b"CATC"), raw("seq2", b"CATC")]);
        model.set_reference(1).unwrap();

        model.clear_reference().unwrap();

        assert_eq!(model.rows().reference(), None);
        assert_eq!(model.view().relative_row_id(1), Some(1));
    }

    #[test]
    fn set_filter_applies_row_pattern() {
        let mut model = alignment_model(vec![
            raw("seq1", b"CATC"),
            raw("seq2", b"CATC"),
            raw("seq3", b"CATC"),
        ]);

        model.set_filter("seq1|seq2".to_string()).unwrap();

        assert_eq!(model.filter().pattern(), Some("seq1|seq2"));
        assert_eq!(model.view().row_count(), 2);
    }

    #[test]
    fn set_filter_clears_pattern_on_empty_string() {
        let mut model = alignment_model(vec![
            raw("seq1", b"CATC"),
            raw("seq2", b"CATC"),
            raw("seq3", b"CATC"),
        ]);
        model.set_filter("seq1|seq2".to_string()).unwrap();

        model.set_filter(String::new()).unwrap();

        assert_eq!(model.filter().pattern(), None);
        assert_eq!(model.view().row_count(), 3);
    }

    #[test]
    fn set_filter_keeps_previous_pattern_on_error() {
        let mut model = alignment_model(vec![raw("seq1", b"CATC"), raw("seq2", b"CATC")]);
        model.set_filter("seq1".to_string()).unwrap();

        let error = model.set_filter("(".to_string()).unwrap_err();

        assert_eq!(model.filter().pattern(), Some("seq1"));
        assert!(matches!(error, libmsa::AlignmentError::InvalidRegex { .. }));
    }

    #[test]
    fn set_gap_filter_hides_filtered_columns() {
        let mut model = alignment_model(vec![
            raw("seq1", b"C--C"),
            raw("seq2", b"C--C"),
            raw("seq3", b"CATC"),
        ]);

        model.set_gap_filter(Some(0.0)).unwrap();

        assert_eq!(model.filter().max_gap_fraction(), Some(0.0));
        assert_eq!(model.view().column_count(), 2);
    }

    #[test]
    fn set_constant_filter_hides_constant_columns() {
        let alignment = libmsa::Alignment::new(vec![
            raw("seq1", b"CN-C"),
            raw("seq2", b"C--C"),
            raw("seq3", b"CTGC"),
        ])
        .unwrap();
        let mut model = AlignmentModel::new(alignment).unwrap();

        model.set_constant_filter(Some(1.0)).unwrap();

        assert_eq!(model.filter().min_constant_fraction(), Some(1.0));
        assert_eq!(model.view().column_count(), 0);
    }

    #[test]
    fn clear_filter_removes_row_and_column_filters() {
        let mut model = alignment_model(vec![
            raw("seq1", b"C--C"),
            raw("seq2", b"C--C"),
            raw("seq3", b"CATC"),
        ]);
        model.set_filter("seq1|seq2".to_string()).unwrap();
        model.set_gap_filter(Some(0.0)).unwrap();
        model.set_constant_filter(Some(1.0)).unwrap();

        model.clear_filter().unwrap();

        assert_eq!(model.filter().pattern(), None);
        assert_eq!(model.filter().max_gap_fraction(), None);
        assert_eq!(model.filter().min_constant_fraction(), None);
        assert_eq!(model.view().row_count(), 3);
        assert_eq!(model.view().column_count(), 4);
    }

    #[test]
    fn set_active_kind_clears_translation_outside_dna() {
        let mut model = alignment_model(vec![raw("seq1", b"CATCATCATCAT")]);
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
    fn set_translation_enables_translation_in_dna() {
        let mut model = alignment_model(vec![raw("seq1", b"CATCATCATCAT")]);

        model
            .set_translation(Some(libmsa::ReadingFrame::Frame2))
            .unwrap();

        assert_eq!(model.translation(), Some(libmsa::ReadingFrame::Frame2));
        assert_eq!(model.translation_frame(), libmsa::ReadingFrame::Frame2);
    }

    #[test]
    fn set_translation_rejects_non_dna_alignment() {
        let mut model = alignment_model(vec![raw("seq1", b"MKF")]);
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
        let mut model = alignment_model(vec![raw("seq1", b"CATCATCATCAT")]);

        model
            .set_translation_frame(libmsa::ReadingFrame::Frame3)
            .unwrap();

        assert_eq!(model.translation(), None);
        assert_eq!(model.translation_frame(), libmsa::ReadingFrame::Frame3);
    }

    #[test]
    fn set_translation_frame_rejects_non_dna_alignment() {
        let mut model = alignment_model(vec![raw("seq1", b"MKF")]);
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
    fn reload_as_protein_preserves_filter_and_restores_dna() {
        let mut model = alignment_model(vec![raw("seq1", b"CATCATCATCAT"), raw("seq2", b"CATCATCATCAT")]);
        model.set_filter("seq1".to_string()).unwrap();
        model
            .set_translation(Some(libmsa::ReadingFrame::Frame2))
            .unwrap();

        model.toggle_reload_as_protein(None).unwrap();

        assert!(model.is_reloaded_as_protein());
        assert_eq!(model.base().active_type(), libmsa::AlignmentType::Protein);
        assert_eq!(model.view().column_count(), 4);
        assert_eq!(model.filter().pattern(), Some("seq1"));
        assert_eq!(model.translation(), None);

        model.toggle_reload_as_protein(None).unwrap();

        assert!(!model.is_reloaded_as_protein());
        assert_eq!(model.base().active_type(), libmsa::AlignmentType::Dna);
        assert_eq!(model.filter().pattern(), Some("seq1"));
        assert_eq!(model.translation(), None);
    }

    #[test]
    fn set_translation_frame_retranslates_reloaded_protein_alignment() {
        let mut model = alignment_model(vec![raw("seq1", b"ATGCCCTAA")]);
        model.toggle_reload_as_protein(None).unwrap();

        assert_eq!(model.base().column_count(), 3);
        assert_eq!(model.base().sequence(0).unwrap().byte_at(0), Some(b'M'));

        model
            .set_translation_frame(libmsa::ReadingFrame::Frame2)
            .unwrap();

        assert!(model.is_reloaded_as_protein());
        assert_eq!(model.translation_frame(), libmsa::ReadingFrame::Frame2);
        assert_eq!(model.base().column_count(), 3);
        assert_eq!(model.base().sequence(0).unwrap().byte_at(0), Some(b'C'));
    }

    #[test]
    fn reload_as_protein_uses_requested_frame_and_keeps_it_global() {
        let mut model = alignment_model(vec![raw("seq1", b"ATGCCCTAA")]);

        model
            .toggle_reload_as_protein(Some(libmsa::ReadingFrame::Frame3))
            .unwrap();

        assert_eq!(model.translation_frame(), libmsa::ReadingFrame::Frame3);
        assert_eq!(model.base().sequence(0).unwrap().byte_at(0), Some(b'A'));

        model.toggle_reload_as_protein(None).unwrap();

        assert_eq!(model.translation_frame(), libmsa::ReadingFrame::Frame3);
    }

    #[test]
    fn filters_changed_in_reloaded_protein_view_persist_in_dna() {
        let mut model = alignment_model(vec![
            raw("seq1", b"CAT---CATCAT"),
            raw("seq2", b"CATCATCATCAT"),
            raw("seq3", b"CATCATCATCAT"),
        ]);
        model.toggle_reload_as_protein(None).unwrap();

        model.set_filter("seq1|seq2".to_string()).unwrap();
        model.set_gap_filter(Some(0.5)).unwrap();
        model.set_constant_filter(Some(1.0)).unwrap();

        assert_eq!(model.filter().pattern(), Some("seq1|seq2"));
        assert_eq!(model.filter().max_gap_fraction(), Some(0.5));
        assert_eq!(model.filter().min_constant_fraction(), Some(1.0));

        model.toggle_reload_as_protein(None).unwrap();

        assert_eq!(model.base().active_type(), libmsa::AlignmentType::Dna);
        assert_eq!(model.filter().pattern(), Some("seq1|seq2"));
        assert_eq!(model.filter().max_gap_fraction(), Some(0.5));
        assert_eq!(model.filter().min_constant_fraction(), Some(1.0));
    }

    #[test]
    fn live_presentation_state_persists_across_reloaded_protein_toggle() {
        let mut model = alignment_model(vec![
            raw("seq1", b"CATCATCATCAT"),
            raw("seq2", b"CATCATCATCAT"),
            raw("seq3", b"CATCATCATCAT"),
        ]);
        model.toggle_reload_as_protein(None).unwrap();

        model.pin(0).unwrap();
        model.set_reference(1).unwrap();
        model.diff_mode = DiffMode::Consensus;
        model.consensus_method = libmsa::ConsensusMethod::Majority;

        model.toggle_reload_as_protein(None).unwrap();

        assert_eq!(model.base().active_type(), libmsa::AlignmentType::Dna);
        assert_eq!(model.rows().pinned(), &[0]);
        assert_eq!(model.rows().reference(), Some(1));
        assert_eq!(model.diff_mode, DiffMode::Consensus);
        assert_eq!(model.consensus_method, libmsa::ConsensusMethod::Majority);
    }

    #[test]
    fn stats_context_keeps_raw_range() {
        let model = alignment_model(vec![raw("seq1", b"CATC"), raw("seq2", b"CATC")]);

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
        let mut model = alignment_model(vec![raw("seq1", b"ATGAAATTT")]);
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
    fn stats_context_returns_none_without_visible_columns() {
        let model = alignment_model(vec![raw("seq1", b"CATC")]);

        assert_eq!(model.stats_context(0..0), None);

        let mut translated = alignment_model(vec![raw("seq1", b"AT")]);
        translated
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap();
        assert_eq!(translated.stats_context(0..2), None);
    }

    #[test]
    fn jump_to_sequence_reports_why_row_is_hidden() {
        let mut model = alignment_model(vec![
            raw("seq1", b"CATC"),
            raw("seq2", b"CATC"),
            raw("seq3", b"CATC"),
            raw("seq4", b"CATC"),
        ]);
        model.pin(0).unwrap();
        model.set_reference(1).unwrap();
        model.set_filter("seq4".to_string()).unwrap();

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
