use std::borrow::Borrow;
use std::sync::Arc;

use regex::Regex;

use crate::error::AlignmentError;
use crate::metrics;
use crate::model::Alignment;
use crate::projection::Projection;

/// Builder for a filtered view over an unfiltered [`Alignment`].
///
/// Row filters are resolved in a fixed order regardless of the order builder
/// methods are called:
///
/// 1. Regex - rows not matching [`Self::with_row_regex`] are removed.
/// 2. Exclusion - explicit excludes ([`Self::without_rows`]) are removed last.
///
/// Column filters ([`Self::with_max_gap_fraction`]) run over the final row set.
#[derive(Debug, Clone)]
pub struct FilterBuilder<'a> {
    source: &'a Alignment,
    row_exclude_sets: Vec<Vec<usize>>,
    row_name_regex: Option<String>,
    max_gap_fraction: Option<f32>,
}

impl<'a> FilterBuilder<'a> {
    /// Excludes the supplied rows from the filtered view.
    pub fn without_rows<I>(mut self, row_ids: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<usize>,
    {
        self.row_exclude_sets
            .push(row_ids.into_iter().map(|row_id| *row_id.borrow()).collect());
        self
    }

    /// Restricts the filtered view to rows whose names match the regex.
    pub fn with_row_regex(mut self, pattern: impl Into<String>) -> Self {
        self.row_name_regex = Some(pattern.into());
        self
    }

    /// Keeps only columns whose gap fraction is at most `threshold`.
    pub fn with_max_gap_fraction(mut self, threshold: f32) -> Self {
        self.max_gap_fraction = Some(threshold);
        self
    }

    /// Resolves all filters and builds a new [`Alignment`]
    pub fn apply(self) -> Result<Alignment, AlignmentError> {
        let row_count = self.source.row_count();
        let column_count = self.source.column_count();

        for row_ids in &self.row_exclude_sets {
            validate_row_ids(row_ids, row_count)?;
        }
        if let Some(max_gap_fraction) = self.max_gap_fraction {
            validate_gap_fraction(max_gap_fraction)?;
        }

        let mut row_ids: Vec<usize> = (0..row_count).collect();
        if let Some(pattern) = &self.row_name_regex {
            let regex = compile_regex(pattern)?;
            row_ids.retain(|&row_id| {
                self.source
                    .sequence_by_absolute(row_id)
                    .is_some_and(|sequence| regex.is_match(sequence.id()))
            });
        }

        let mut membership = vec![true; row_count];
        for exclude_set in &self.row_exclude_sets {
            for &row_id in exclude_set {
                membership[row_id] = false;
            }
        }
        row_ids.retain(|&row_id| membership[row_id]);

        let mut column_ids: Vec<usize> = (0..column_count).collect();
        if let Some(max_gap_fraction) = self.max_gap_fraction {
            let temp_rows = Projection::Filtered(Arc::from(row_ids.as_slice()));
            let gap_fractions = metrics::counted_columns_range(
                &self.source.data,
                &temp_rows,
                &self.source.columns,
                0..column_count,
            )
            .map(|columns| metrics::gap_fraction_from_columns(&columns))?;

            column_ids.retain(|&column_id| {
                gap_fractions
                    .get(column_id)
                    .is_none_or(|(_, gap_fraction)| *gap_fraction <= max_gap_fraction)
            });
        }

        let rows_proj = if row_ids.len() == row_count {
            Projection::Full { len: row_count }
        } else {
            Projection::Filtered(Arc::from(row_ids))
        };

        let cols_proj = if column_ids.len() == column_count {
            Projection::Full { len: column_count }
        } else {
            Projection::Filtered(Arc::from(column_ids))
        };

        Ok(Alignment::from_selection(
            Arc::clone(&self.source.data),
            self.source.detected_type(),
            self.source.active_type(),
            rows_proj,
            cols_proj,
        ))
    }
}

impl<'a> FilterBuilder<'a> {
    pub(crate) fn new(source: &'a Alignment) -> Self {
        Self {
            source,
            row_exclude_sets: Vec::new(),
            row_name_regex: None,
            max_gap_fraction: None,
        }
    }
}

fn compile_regex(pattern: &str) -> Result<Regex, AlignmentError> {
    Regex::new(pattern).map_err(|error| AlignmentError::InvalidRegex {
        pattern: pattern.to_string(),
        source: error,
    })
}

fn validate_row_ids(row_ids: &[usize], row_count: usize) -> Result<(), AlignmentError> {
    let mut seen = vec![false; row_count];
    for &row_id in row_ids {
        if row_id >= row_count {
            return Err(AlignmentError::RowOutOfBounds {
                index: row_id,
                row_count,
            });
        }
        if std::mem::replace(&mut seen[row_id], true) {
            return Err(AlignmentError::DuplicateRowIndex { index: row_id });
        }
    }
    Ok(())
}

fn validate_gap_fraction(threshold: f32) -> Result<(), AlignmentError> {
    if threshold.is_finite() && (0.0..=1.0).contains(&threshold) {
        return Ok(());
    }

    Err(AlignmentError::InvalidGapFraction(threshold))
}

#[cfg(test)]
mod filter_builder_tests {
    use crate::{
        Alignment, AlignmentError, AlignmentType, ColumnSummary, ConsensusMethod, RawSequence,
    };

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn dna_alignment(rows: &[(&str, &[u8])]) -> Alignment {
        Alignment::new_with_type(
            rows.iter()
                .map(|(id, seq)| raw(id, seq))
                .collect::<Vec<_>>(),
            AlignmentType::Dna,
        )
        .unwrap()
    }

    fn generic_alignment(rows: &[(&str, &[u8])]) -> Alignment {
        Alignment::new_with_type(
            rows.iter()
                .map(|(id, seq)| raw(id, seq))
                .collect::<Vec<_>>(),
            AlignmentType::Generic,
        )
        .unwrap()
    }

    #[test]
    fn filter_builder_is_order_insensitive() {
        let alignment = dna_alignment(&[
            ("ref", b"A-"),
            ("keep-a", b"AA"),
            ("keep-b", b"AA"),
            ("drop", b"--"),
        ]);

        let first = alignment
            .filter()
            .unwrap()
            .without_rows([0])
            .with_max_gap_fraction(0.0)
            .apply()
            .unwrap();
        let second = alignment
            .filter()
            .unwrap()
            .with_max_gap_fraction(0.0)
            .without_rows([0])
            .apply()
            .unwrap();

        assert_eq!(first.row_count(), second.row_count());
        assert_eq!(first.column_count(), second.column_count());
        let first_ids: Vec<_> = first.absolute_row_ids().collect();
        let second_ids: Vec<_> = second.absolute_row_ids().collect();
        assert_eq!(first_ids, second_ids);
    }

    #[test]
    fn filter_supports_regex_and_index_filters() {
        let alignment = generic_alignment(&[
            ("ref", b"AC"),
            ("sample-1", b"AC"),
            ("sample-2", b"TC"),
            ("other", b"GC"),
        ]);
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([0])
            .with_row_regex("^sample")
            .apply()
            .unwrap();

        let ids: Vec<_> = filtered.absolute_row_ids().collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn filter_supports_regex_filters() {
        let alignment =
            generic_alignment(&[("sample-1", b"AC"), ("sample-2", b"TC"), ("control", b"GC")]);
        let filtered = alignment
            .filter()
            .unwrap()
            .with_row_regex("^sample-\\d$")
            .apply()
            .unwrap();

        let ids: Vec<_> = filtered.absolute_row_ids().collect();
        assert_eq!(ids, vec![0, 1]);
    }

    #[test]
    fn invalid_regex_returns_error() {
        let err = generic_alignment(&[("sample-1", b"AC")])
            .filter()
            .unwrap()
            .with_row_regex("[")
            .apply()
            .unwrap_err();

        assert!(matches!(err, AlignmentError::InvalidRegex { .. }));
    }

    #[test]
    fn invalid_gap_fraction_returns_error() {
        let err = generic_alignment(&[("sample-1", b"AC")])
            .filter()
            .unwrap()
            .with_max_gap_fraction(1.5)
            .apply()
            .unwrap_err();

        assert_eq!(err, AlignmentError::InvalidGapFraction(1.5));
    }

    #[test]
    fn gap_fraction_filter_uses_filtered_rows() {
        let alignment = dna_alignment(&[("ref", b"A"), ("s1", b"A"), ("s2", b"A"), ("s3", b"-")]);
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([0, 3])
            .with_max_gap_fraction(0.0)
            .apply()
            .unwrap();

        let col_ids: Vec<_> = filtered.absolute_column_ids().collect();
        assert_eq!(col_ids, vec![0]);
    }

    #[test]
    fn column_summaries_positions_empty_returns_empty() {
        let alignment = generic_alignment(&[("s1", b"A-"), ("s2", b"--")]);

        assert_eq!(
            alignment
                .column_summaries_positions(&[], ConsensusMethod::MajorityNonGap)
                .unwrap(),
            Vec::<ColumnSummary>::new()
        );
    }

    #[test]
    fn filtered_alignment_rejects_chained_filter() {
        let alignment = dna_alignment(&[("s1", b"AC"), ("s2", b"TG")]);
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([0])
            .apply()
            .unwrap();
        assert!(filtered.filter().is_err());
    }

    #[test]
    fn full_projection_kept() {
        let alignment = dna_alignment(&[("s1", b"AC"), ("s2", b"TG")]);
        let filtered = alignment.filter().unwrap().apply().unwrap();
        assert!(!filtered.is_filtered());
    }
}

#[cfg(test)]
mod filtered_alignment_behaviour_tests {
    use crate::{Alignment, AlignmentType, ConsensusMethod, RawSequence};

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn dna_alignment(rows: &[(&str, &[u8])]) -> Alignment {
        Alignment::new_with_type(
            rows.iter()
                .map(|(id, seq)| raw(id, seq))
                .collect::<Vec<_>>(),
            AlignmentType::Dna,
        )
        .unwrap()
    }

    fn generic_alignment(rows: &[(&str, &[u8])]) -> Alignment {
        Alignment::new_with_type(
            rows.iter()
                .map(|(id, seq)| raw(id, seq))
                .collect::<Vec<_>>(),
            AlignmentType::Generic,
        )
        .unwrap()
    }

    #[test]
    fn filter_sequence_iteration_uses_absolute_ids() {
        let alignment = generic_alignment(&[("ref", b"AA"), ("s1", b"AA"), ("s2", b"TT")]);
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([0])
            .apply()
            .unwrap();

        assert_eq!(
            filtered.sequence_by_absolute(1).unwrap().absolute_row_id(),
            1
        );
        assert_eq!(filtered.sequence_by_absolute(1).unwrap().id(), "s1");
        assert_eq!(
            filtered.sequence_by_absolute(2).unwrap().absolute_row_id(),
            2
        );
        assert_eq!(filtered.sequence_by_absolute(2).unwrap().id(), "s2");
    }

    #[test]
    fn filter_consensus_uses_selected_rows() {
        let alignment =
            generic_alignment(&[("ref", b"TA"), ("s1", b"AC"), ("s2", b"AC"), ("s3", b"TC")]);

        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([0])
            .apply()
            .unwrap();
        let consensus = filtered
            .consensus_positions(&[0, 1], ConsensusMethod::MajorityNonGap)
            .unwrap();

        assert_eq!(consensus, vec![(0, Some(b'A')), (1, Some(b'C'))]);
    }

    #[test]
    fn filter_conservation_uses_selected_rows() {
        let alignment = dna_alignment(&[("s1", b"A"), ("s2", b"-"), ("s3", b"A")]);
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([1])
            .apply()
            .unwrap();

        assert_eq!(filtered.conservation_positions(&[0]), Ok(vec![(0, 1.0)]));
    }
}
