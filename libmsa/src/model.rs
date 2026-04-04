use std::ops::Range;
use std::sync::Arc;

use rand::{SeedableRng, rngs::StdRng};

use crate::alignment_type::AlignmentType;
use crate::data::{AlignmentData, RawSequence};
use crate::detection::{DetectionOptions, detect_alignment_type};
use crate::error::AlignmentError;
use crate::filter::FilterBuilder;
use crate::projection::Projection;
use crate::translation::{ReadingFrame, TranslatedAlignment, TranslationTable};

const DETECTION_SEED: u64 = u64::from_be_bytes(*b"REDRIGHT");

/// A multiple sequence alignment.
///
/// `Alignment` stores a set of equal-length sequences together with
/// the current view over that data. The view can expose all rows and columns or
/// a filtered projection, while still preserving absolute row and column
/// coordinates into the underlying alignment.
#[derive(Debug, Clone)]
pub struct Alignment {
    pub(crate) data: Arc<AlignmentData>,
    detected_type: AlignmentType,
    active_type: AlignmentType,
    pub(crate) rows: Projection,
    pub(crate) columns: Projection,
}

impl Alignment {
    /// Creates an alignment from raw sequences and detects its kind using the default detection options.
    ///
    /// The returned alignment starts with all rows and columns visible. The detected kind becomes both the
    /// detected kind and the active kind for the new alignment.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::Empty`] if `seqs` is empty.
    ///
    /// [`AlignmentError::EmptySequence`] if any sequence in `seqs` has an empty sequence.
    ///
    /// [`AlignmentError::LengthMismatch`] if the sequences in `seqs` do not all have the same length.
    pub fn new(seqs: impl IntoIterator<Item = RawSequence>) -> Result<Self, AlignmentError> {
        Self::new_with_detection_options(seqs, DetectionOptions::default())
    }

    /// Creates an alignment from raw sequences and detects its kind using the supplied detection options.
    ///
    /// The returned alignment starts with all rows and columns visible. The detected kind becomes both the
    /// detected kind and the active kind for the new alignment.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::Empty`] if `seqs` is empty.
    ///
    /// [`AlignmentError::EmptySequence`] if any sequence in `seqs` has an empty sequence.
    ///
    /// [`AlignmentError::LengthMismatch`] if the sequences in `seqs` do not all have the same length.
    pub fn new_with_detection_options(
        seqs: impl IntoIterator<Item = RawSequence>,
        options: DetectionOptions,
    ) -> Result<Self, AlignmentError> {
        let data = data_from_raw_sequences(seqs)?;
        let mut rng = StdRng::seed_from_u64(DETECTION_SEED);
        let detected = detect_alignment_type(&data, options, &mut rng);
        Ok(Self::from_data(data, detected))
    }

    /// Creates an alignment from raw sequences with an explicit type.
    ///
    /// This constructor skips type detection. The returned alignment starts with all rows and columns
    /// visible, and the supplied `kind` is recorded as both the detected kind and the active kind.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::Empty`] if `seqs` is empty.
    ///
    /// [`AlignmentError::EmptySequence`] if any sequence in `seqs` has an empty sequence.
    ///
    /// [`AlignmentError::LengthMismatch`] if the sequences in `seqs` do not all have the same length.
    pub(crate) fn new_with_type(
        seqs: impl IntoIterator<Item = RawSequence>,
        kind: AlignmentType,
    ) -> Result<Self, AlignmentError> {
        let data = data_from_raw_sequences(seqs)?;
        Ok(Self::from_data(data, kind))
    }

    pub(crate) fn from_data(data: AlignmentData, alignment_type: AlignmentType) -> Self {
        let rows = Projection::Full {
            len: data.sequences.len(),
        };
        let columns = Projection::Full { len: data.length };
        Self {
            data: Arc::new(data),
            detected_type: alignment_type,
            active_type: alignment_type,
            rows,
            columns,
        }
    }

    pub(crate) fn with_projections(&self, rows: Projection, columns: Projection) -> Self {
        Self {
            data: Arc::clone(&self.data),
            detected_type: self.detected_type,
            active_type: self.active_type,
            rows,
            columns,
        }
    }

    /// Returns the number of visible sequences.
    ///
    /// This is the length of the alignment's current row projection. For a filtered alignment, it
    /// returns the number of rows that remain visible after filtering.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the number of visible columns.
    ///
    /// This is the length of the alignment's current column projection. For a filtered alignment, it
    /// returns only the columns that remain visible after filtering.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Returns the length in characters of the longest visible sequence identifier, or `0` if no sequences are visible.
    pub fn max_id_len(&self) -> usize {
        self.rows
            .iter()
            .map(|abs_row| {
                self.data
                    .sequences
                    .get(abs_row)
                    .expect("selected row must exist")
                    .id
                    .chars()
                    .count()
            })
            .max()
            .unwrap_or(0)
    }

    /// Returns a [`RowView`] for the visible sequence at `relative_row`.
    ///
    /// The row index is relative to this alignment's current row projection, so `0` refers to the first
    /// visible sequence rather than the first sequence in the underlying data. The returned
    /// [`RowView`] also uses this alignment's current column projection and active kind.
    ///
    /// Returns `None` if `relative_row` does not refer to a visible row.
    pub fn sequence(&self, relative_row: usize) -> Option<RowView<'_>> {
        let abs_row = self.rows.absolute(relative_row)?;
        let seq = self.data.sequences.get(abs_row)?;
        Some(RowView {
            absolute_row_id: abs_row,
            id: &seq.id,
            data: &seq.sequence,
            columns: &self.columns,
        })
    }

    /// Returns a [`RowView`] for the visible sequence at `absolute_row`.
    ///
    /// The row index refers to the underlying alignment data rather than this alignment's current row
    /// projection. The returned [`RowView`] is produced only if that absolute row is still visible
    /// in this alignment, and it uses this alignment's current column projection and active kind.
    ///
    /// Returns `None` if `absolute_row` is out of bounds or refers to a row that is not visible.
    pub(crate) fn sequence_by_absolute(&self, absolute_row: usize) -> Option<RowView<'_>> {
        self.rows.relative(absolute_row)?;
        let seq = self.data.sequences.get(absolute_row)?;
        Some(RowView {
            absolute_row_id: absolute_row,
            id: &seq.id,
            data: &seq.sequence,
            columns: &self.columns,
        })
    }

    /// Returns a [`RowView`] for the absolute row but projected
    /// through this alignment's current column projection.
    ///
    /// Unlike [`sequence_by_absolute`], this method does not require `abs_row`
    /// to be visible in the current row projection.
    ///
    /// Returns `None` only when `abs_row` is out of bounds for the underlying
    /// alignment data.
    pub fn project_absolute_row(&self, abs_row: usize) -> Option<RowView<'_>> {
        let seq = self.data.sequences.get(abs_row)?;
        Some(RowView {
            absolute_row_id: abs_row,
            id: &seq.id,
            data: &seq.sequence,
            columns: &self.columns,
        })
    }

    /// Returns the absolute row index for `relative`, or `None` if `relative` is not visible.
    pub fn absolute_row_id(&self, relative: usize) -> Option<usize> {
        self.rows.absolute(relative)
    }

    /// Returns the absolute column index for `relative`, or `None` if `relative` is not visible.
    pub fn absolute_column_id(&self, relative: usize) -> Option<usize> {
        self.columns.absolute(relative)
    }

    /// Returns an iterator over the visible rows absolute index.
    pub(crate) fn absolute_row_ids(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        self.rows.iter()
    }

    /// Returns an iterator over the visible columns absolute index.
    pub fn absolute_column_ids(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        self.columns.iter()
    }

    /// Returns the relative row index for `absolute`, or `None` if that row is not visible.
    pub fn relative_row_id(&self, absolute: usize) -> Option<usize> {
        self.rows.relative(absolute)
    }

    /// Returns the relative column index for `absolute`, or `None` if that column is not visible.
    pub fn relative_column_id(&self, absolute: usize) -> Option<usize> {
        self.columns.relative(absolute)
    }

    /// Returns the type currently used to interpret this alignment.
    pub fn active_type(&self) -> AlignmentType {
        self.active_type
    }

    /// Sets the active type override to `alignment_type`.
    pub fn set_override_type(&mut self, alignment_type: AlignmentType) {
        self.active_type = alignment_type;
    }

    /// Creates a lazy translated view over this alignment with a specific translation table.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::UnsupportedOperation`] if the active kind does not support translation.
    ///
    /// [`AlignmentError::UnsupportedOperation`] if this alignment has a filtered column
    /// projection.
    ///
    /// [`AlignmentError::TranslationEmpty`] if the chosen reading frame produces no translated
    /// residues.
    pub fn translated_with(
        &self,
        frame: ReadingFrame,
        table: TranslationTable,
    ) -> Result<TranslatedAlignment<'_>, AlignmentError> {
        TranslatedAlignment::new(self, frame, table)
    }

    /// Creates a lazy translated view over this alignment with the standard translation table.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::UnsupportedOperation`] if the active kind does not support translation.
    ///
    /// [`AlignmentError::UnsupportedOperation`] if this alignment has a filtered column
    /// projection.
    ///
    /// [`AlignmentError::TranslationEmpty`] if the chosen reading frame produces no translated
    /// residues.
    pub fn translated(
        &self,
        frame: ReadingFrame,
    ) -> Result<TranslatedAlignment<'_>, AlignmentError> {
        self.translated_with(frame, TranslationTable::STANDARD)
    }

    /// Returns a [`FilterBuilder`] for creating a filtered view of this alignment.
    ///
    /// Filtered views can only be started from an unfiltered alignment.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::UnsupportedOperation`] if this alignment is already filtered.
    pub fn filter(&self) -> Result<FilterBuilder<'_>, AlignmentError> {
        if self.is_filtered() {
            return Err(AlignmentError::UnsupportedOperation {
                operation: "filter (already filtered)",
                kind: self.active_type,
            });
        }

        Ok(FilterBuilder::new(self))
    }

    /// Returns `true` if this alignment has been filtered.
    pub fn is_filtered(&self) -> bool {
        !self.rows.is_full() || !self.columns.is_full()
    }
}

/// A borrowed view of one sequence row within an [`Alignment`].
///
/// `RowView` does not own sequence data. Instead, it exposes a single row
/// from an alignment together with that alignment's current column projection
/// and active kind. This means its column-based accessors operate on the
/// visible columns of the parent alignment rather than the full
/// underlying sequence.
#[derive(Debug, Clone, Copy)]
pub struct RowView<'a> {
    absolute_row_id: usize,
    id: &'a str,
    data: &'a [u8],
    columns: &'a Projection,
}

impl<'a> RowView<'a> {
    /// Returns the absolute row index of this sequence.
    pub fn absolute_row_id(&self) -> usize {
        self.absolute_row_id
    }

    /// Returns the sequence identifier.
    pub fn id(&self) -> &str {
        self.id
    }

    /// Returns the number of visible columns in this sequence view.
    ///
    /// This reflects the column projection of the alignment that produced this view,
    /// not the full length of the underlying sequence data.
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Returns the byte at `relative_col`, or `None` if the column is out of bounds.
    ///
    /// The column index is relative to this view's column projection.
    pub fn byte_at(&self, relative_col: usize) -> Option<u8> {
        let abs_col = self.columns.absolute(relative_col)?;
        Some(self.data[abs_col])
    }

    /// Returns an iterator over `(absolute_column, byte)` pairs for the given relative column range.
    ///
    /// The range is relative to this view's column projection. Each yielded pair carries the
    /// absolute column index, which identifies the position in the underlying sequence data.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::EmptyRange`] if `range` is empty.
    ///
    /// [`AlignmentError::ColumnOutOfBounds`] if `range.end` exceeds the number of visible columns.
    pub fn indexed_bytes_range(
        &self,
        range: Range<usize>,
    ) -> Result<impl Iterator<Item = (usize, u8)> + '_, AlignmentError> {
        if range.is_empty() {
            return Err(AlignmentError::EmptyRange);
        }

        if range.end > self.columns.len() {
            return Err(AlignmentError::ColumnOutOfBounds {
                index: range.end - 1,
                length: self.columns.len(),
            });
        }

        let columns = self.columns;
        let data = self.data;
        Ok(range.map(move |rel_col| {
            let abs_col = columns.absolute(rel_col).expect("validated range");
            (abs_col, data[abs_col])
        }))
    }
}

fn data_from_raw_sequences(
    sequences: impl IntoIterator<Item = RawSequence>,
) -> Result<AlignmentData, AlignmentError> {
    let sequences = sequences
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, _>>()?;
    AlignmentData::new(sequences)
}

#[cfg(test)]
mod alignment_construction_tests {
    use super::*;
    use crate::alignment_type::AlignmentType;

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    #[test]
    fn constructs_valid_alignment() {
        let alignment = Alignment::new(vec![raw("seq-1", b"ACGT"), raw("seq-2", b"TGCA")]).unwrap();
        assert_eq!(alignment.column_count(), 4);
        assert_eq!(alignment.row_count(), 2);
        assert_eq!(alignment.active_type(), AlignmentType::Dna);
    }

    #[test]
    fn new_with_kind_skips_detection() {
        let alignment = Alignment::new_with_type(
            vec![raw("seq-1", b"ACGT"), raw("seq-2", b"TGCA")],
            AlignmentType::Protein,
        )
        .unwrap();
        assert_eq!(alignment.active_type(), AlignmentType::Protein);
    }

    #[test]
    fn rejects_empty_alignment() {
        let result = Alignment::new(vec![]);
        assert!(matches!(result, Err(AlignmentError::Empty)));
    }

    #[test]
    fn rejects_mismatched_lengths() {
        let result = Alignment::new(vec![raw("seq-1", b"ACGT"), raw("seq-2", b"ACG")]);
        assert!(matches!(result, Err(AlignmentError::LengthMismatch { .. })));
    }

    #[test]
    fn override_type_method_updates_active_type() {
        let mut alignment =
            Alignment::new(vec![raw("seq-1", b"ACGT"), raw("seq-2", b"TGCA")]).unwrap();

        alignment.set_override_type(AlignmentType::Protein);
        assert_eq!(alignment.active_type(), AlignmentType::Protein);
    }

    #[test]
    fn translated_rejects_non_dna() {
        let alignment = Alignment::new_with_type(
            vec![raw("seq-1", b"MFPQ"), raw("seq-2", b"WLYH")],
            AlignmentType::Protein,
        )
        .unwrap();

        assert!(matches!(
            alignment.translated(ReadingFrame::Frame1),
            Err(AlignmentError::UnsupportedOperation {
                operation: "translate",
                kind: AlignmentType::Protein,
            })
        ));
    }

    #[test]
    fn is_filtered_false_on_new_alignment() {
        let alignment = Alignment::new(vec![raw("s1", b"AC")]).unwrap();
        assert!(!alignment.is_filtered());
    }
}

#[cfg(test)]
mod alignment_access_tests {
    use super::*;

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    #[test]
    fn getters_work() {
        let alignment = Alignment::new(vec![
            raw("seq-1", b"AAAA"),
            raw("seq-2", b"CCCC"),
            raw("seq-3", b"GGGG"),
        ])
        .unwrap();

        let second = alignment.sequence(1).unwrap();
        assert_eq!(second.id(), "seq-2");
        assert_eq!(second.len(), 4);
        assert!(alignment.sequence(99).is_none());
        assert_eq!(alignment.sequence(0).unwrap().id(), "seq-1");
        assert_eq!(alignment.sequence(2).unwrap().id(), "seq-3");
    }

    #[test]
    fn sequence_view_byte_at() {
        let alignment = Alignment::new(vec![raw("s1", b"ACGT")]).unwrap();
        let sv = alignment.sequence(0).unwrap();

        assert_eq!(sv.byte_at(0), Some(b'A'));
        assert_eq!(sv.byte_at(3), Some(b'T'));
        assert_eq!(sv.byte_at(4), None);
    }

    #[test]
    #[should_panic(expected = "selected row must exist")]
    fn max_id_len_panics_for_invalid_row_projection() {
        let alignment = Alignment::new(vec![raw("s1", b"AC")]).unwrap();
        let filtered = alignment.with_projections(
            Projection::Filtered(Arc::from(vec![1usize])),
            Projection::Full {
                len: alignment.column_count(),
            },
        );

        filtered.max_id_len();
    }

    #[test]
    fn sequence_by_absolute_full() {
        let alignment = Alignment::new(vec![raw("s1", b"AC"), raw("s2", b"TG")]).unwrap();
        let sv = alignment.sequence_by_absolute(1).unwrap();
        assert_eq!(sv.id, "s2");
        assert!(alignment.sequence_by_absolute(2).is_none());
    }

    #[test]
    fn sequence_by_absolute_filtered() {
        let alignment =
            Alignment::new(vec![raw("s1", b"AC"), raw("s2", b"TG"), raw("s3", b"AA")]).unwrap();
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([1])
            .apply()
            .unwrap();

        assert_eq!(filtered.sequence_by_absolute(0).unwrap().id(), "s1");
        assert_eq!(filtered.sequence_by_absolute(2).unwrap().id(), "s3");
        assert!(filtered.sequence_by_absolute(1).is_none());
        assert!(filtered.sequence_by_absolute(99).is_none());
    }

    #[test]
    fn indexed_bytes_range_full() {
        let alignment = Alignment::new(vec![raw("s1", b"ACGT")]).unwrap();
        let sv = alignment.sequence(0).unwrap();
        let pairs: Vec<_> = sv.indexed_bytes_range(1..3).unwrap().collect();
        assert_eq!(pairs, vec![(1, b'C'), (2, b'G')]);
    }

    #[test]
    fn indexed_bytes_range_filtered() {
        let alignment = Alignment::new(vec![raw("s1", b"ACGT")]).unwrap();
        let filtered = alignment.with_projections(
            Projection::Full {
                len: alignment.row_count(),
            },
            Projection::Filtered(Arc::from(vec![0, 2, 3])),
        );
        let sv = filtered.sequence(0).unwrap();
        let pairs: Vec<_> = sv.indexed_bytes_range(0..2).unwrap().collect();
        assert_eq!(pairs, vec![(0, b'A'), (2, b'G')]);
    }

    #[test]
    fn indexed_bytes_range_empty_error() {
        let alignment = Alignment::new(vec![raw("s1", b"ACGT")]).unwrap();
        let sv = alignment.sequence(0).unwrap();
        assert!(matches!(
            sv.indexed_bytes_range(2..2),
            Err(AlignmentError::EmptyRange)
        ));
    }

    #[test]
    fn indexed_bytes_range_out_of_bounds() {
        let alignment = Alignment::new(vec![raw("s1", b"ACGT")]).unwrap();
        let sv = alignment.sequence(0).unwrap();
        assert!(matches!(
            sv.indexed_bytes_range(2..5),
            Err(AlignmentError::ColumnOutOfBounds {
                index: 4,
                length: 4
            })
        ));
    }
}

#[cfg(test)]
mod alignment_projection_tests {
    use super::*;

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn filtered_alignment() -> (Alignment, Alignment) {
        let base = Alignment::new(vec![
            raw("s1", b"ACGT"),
            raw("s2", b"TTTT"),
            raw("s3", b"GGGG"),
        ])
        .unwrap();
        let filtered = base.filter().unwrap().without_rows([1]).apply().unwrap();
        (base, filtered)
    }

    fn indexed_bytes(view: RowView<'_>) -> Vec<(usize, u8)> {
        view.indexed_bytes_range(0..view.len()).unwrap().collect()
    }

    #[test]
    fn relative_row_id_full() {
        let alignment = Alignment::new(vec![raw("s1", b"AC"), raw("s2", b"TG")]).unwrap();
        assert_eq!(alignment.relative_row_id(0), Some(0));
        assert_eq!(alignment.relative_row_id(1), Some(1));
        assert_eq!(alignment.relative_row_id(2), None);
    }

    #[test]
    fn relative_column_id_full() {
        let alignment = Alignment::new(vec![raw("s1", b"ACGT")]).unwrap();
        assert_eq!(alignment.relative_column_id(0), Some(0));
        assert_eq!(alignment.relative_column_id(3), Some(3));
        assert_eq!(alignment.relative_column_id(4), None);
    }

    #[test]
    fn relative_absolute_ids_filtered() {
        let alignment = Alignment::new(vec![
            raw("s1", b"ACGT"),
            raw("s2", b"TGCA"),
            raw("s3", b"AAAA"),
        ])
        .unwrap();
        let filtered = alignment.with_projections(
            Projection::Filtered(Arc::from(vec![0, 2])),
            Projection::Filtered(Arc::from(vec![1, 3])),
        );

        assert_eq!(filtered.relative_row_id(0), Some(0));
        assert_eq!(filtered.relative_row_id(2), Some(1));
        assert_eq!(filtered.relative_row_id(1), None);

        assert_eq!(filtered.relative_column_id(1), Some(0));
        assert_eq!(filtered.relative_column_id(3), Some(1));
        assert_eq!(filtered.relative_column_id(0), None);

        assert_eq!(filtered.absolute_row_id(0), Some(0));
        assert_eq!(filtered.absolute_row_id(1), Some(2));
        assert_eq!(filtered.absolute_row_id(2), None);

        assert_eq!(filtered.absolute_column_id(0), Some(1));
        assert_eq!(filtered.absolute_column_id(1), Some(3));
        assert_eq!(filtered.absolute_column_id(2), None);
    }

    #[test]
    fn unfiltered_matches_sequence() {
        let alignment = Alignment::new(vec![raw("s1", b"ACGT"), raw("s2", b"TGCA")]).unwrap();

        for relative in 0..alignment.row_count() {
            let via_sequence = alignment.sequence(relative).unwrap();
            let via_project = alignment
                .project_absolute_row(alignment.absolute_row_id(relative).unwrap())
                .unwrap();

            assert_eq!(
                via_sequence.absolute_row_id(),
                via_project.absolute_row_id()
            );
            assert_eq!(via_sequence.id(), via_project.id());
            assert_eq!(via_sequence.len(), via_project.len());
            assert_eq!(indexed_bytes(via_sequence), indexed_bytes(via_project));
        }
    }

    #[test]
    fn returns_excluded_row() {
        let (_, filtered) = filtered_alignment();

        assert!(filtered.sequence_by_absolute(1).is_none());

        let sv = filtered.project_absolute_row(1).unwrap();
        assert_eq!(sv.id, "s2");
        assert_eq!(sv.absolute_row_id(), 1);
    }

    #[test]
    fn visible_rows_match_sequence_by_absolute() {
        let (_, filtered) = filtered_alignment();

        for abs in [0usize, 2] {
            let via_sba = filtered.sequence_by_absolute(abs).unwrap();
            let via_proj = filtered.project_absolute_row(abs).unwrap();

            assert_eq!(via_sba.id, via_proj.id);
            assert_eq!(via_sba.absolute_row_id(), via_proj.absolute_row_id());
            assert_eq!(via_sba.len(), via_proj.len());
            assert_eq!(indexed_bytes(via_sba), indexed_bytes(via_proj));
        }
    }

    #[test]
    fn column_projection_applied_to_excluded_row() {
        let base = Alignment::new(vec![raw("visible", b"ACGT"), raw("excluded", b"TTCA")]).unwrap();
        let col_filtered = base.with_projections(
            Projection::Filtered(Arc::from(vec![0usize])),
            Projection::Filtered(Arc::from(vec![0usize, 2])),
        );

        let sv = col_filtered.project_absolute_row(1).unwrap();
        let pairs = indexed_bytes(sv);

        assert_eq!(sv.len(), 2);
        assert_eq!(sv.byte_at(0), Some(b'T'));
        assert_eq!(sv.byte_at(1), Some(b'C'));
        assert_eq!(pairs, vec![(0, b'T'), (2, b'C')]);
        assert_eq!(
            pairs.iter().map(|(column, _)| *column).collect::<Vec<_>>(),
            col_filtered.absolute_column_ids().collect::<Vec<_>>()
        );
    }

    #[test]
    fn out_of_bounds_returns_none() {
        let alignment = Alignment::new(vec![raw("s1", b"AC"), raw("s2", b"TG")]).unwrap();
        let (_, filtered) = filtered_alignment();

        assert!(alignment.project_absolute_row(2).is_none());
        assert!(alignment.project_absolute_row(99).is_none());
        assert!(filtered.project_absolute_row(3).is_none());
        assert!(filtered.project_absolute_row(99).is_none());
    }
}
