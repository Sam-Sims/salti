use rayon::prelude::*;

use crate::{Grid, counts::count_blocks, metrics};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColumnFilter {
    pub max_gap_fraction: Option<f32>,
    pub min_const_fraction: Option<f32>,
}

impl ColumnFilter {
    /// Returns true if any filter is active
    pub fn is_active(self) -> bool {
        self.max_gap_fraction.is_some() || self.min_const_fraction.is_some()
    }
}

impl Grid<'_> {
    /// Returns the ids of any columns passing the filter.
    ///
    /// No filter = every row is kept
    pub fn kept_columns(self, rows: &[usize], filter: ColumnFilter) -> Vec<usize> {
        let cols: Vec<usize> = (0..self.width()).collect();
        if !filter.is_active() {
            return cols;
        }

        let alignment_type = self.alignment_type();
        count_blocks(self, rows, &cols)
            .flat_map_iter(|(block, counts)| {
                block
                    .iter()
                    .zip(counts)
                    .filter(move |(_, counts)| {
                        let gap_ok = filter.max_gap_fraction.is_none_or(|threshold| {
                            metrics::gap_fraction_from_counts(counts) <= threshold
                        });
                        let const_ok = filter.min_const_fraction.is_none_or(|threshold| {
                            metrics::max_counted_symbol_fraction_from_counts(counts, alignment_type)
                                .is_none_or(|fraction| fraction < threshold)
                        });
                        gap_ok && const_ok
                    })
                    .map(|(&col, _)| col)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::ColumnFilter;
    use crate::{Alignment, AlignmentType, RawSequence, ReadingFrame};

    fn dna(sequences: &[&[u8]]) -> Alignment {
        Alignment::new_with_type(
            sequences
                .iter()
                .enumerate()
                .map(|(i, sequence)| RawSequence {
                    id: format!("s{i}"),
                    sequence: sequence.to_vec(),
                }),
            AlignmentType::Dna,
        )
        .unwrap()
    }

    fn sample() -> Alignment {
        dna(&[b"AAAANN", b"AACANN", b"A-GC-N", b"A-T-A-"])
    }

    fn filter(max_gap_fraction: Option<f32>, min_const_fraction: Option<f32>) -> ColumnFilter {
        ColumnFilter {
            max_gap_fraction,
            min_const_fraction,
        }
    }

    #[rstest]
    #[case::inactive(&[0, 1, 2, 3], filter(None, None), &[0, 1, 2, 3, 4, 5])]
    #[case::gap_zero(&[0, 1, 2, 3], filter(Some(0.0), None), &[0, 2])]
    #[case::gap_at_threshold_kept(&[0, 1, 2, 3], filter(Some(0.25), None), &[0, 2, 3, 4, 5])]
    #[case::const_below_threshold_kept(&[0, 1, 2, 3], filter(None, Some(1.0)), &[2, 3, 5])]
    #[case::const_at_threshold_dropped(&[0, 1, 2, 3], filter(None, Some(2.0 / 3.0)), &[2, 5])]
    #[case::gap_and_const(&[0, 1, 2, 3], filter(Some(0.0), Some(1.0)), &[2])]
    #[case::subset_rows(&[0, 1], filter(None, Some(1.0)), &[2, 4, 5])]
    #[case::no_rows(&[], filter(Some(0.0), Some(0.5)), &[0, 1, 2, 3, 4, 5])]
    fn kept_columns_works(
        #[case] rows: &[usize],
        #[case] filter: ColumnFilter,
        #[case] expected: &[usize],
    ) {
        let alignment = sample();
        assert_eq!(
            alignment
                .grid(AlignmentType::Dna)
                .kept_columns(rows, filter),
            expected
        );
    }

    #[rstest]
    #[case::dna(AlignmentType::Dna, &[2, 3, 5])]
    #[case::generic(AlignmentType::Generic, &[2, 3, 4])]
    fn kept_columns_uses_grid_alignment_type(
        #[case] alignment_type: AlignmentType,
        #[case] expected: &[usize],
    ) {
        let alignment = sample();
        assert_eq!(
            alignment
                .grid(alignment_type)
                .kept_columns(&[0, 1, 2, 3], filter(None, Some(1.0))),
            expected
        );
    }

    #[test]
    fn kept_columns_keeps_ids_across_blocks() {
        let row: Vec<u8> = b"-AC".iter().copied().cycle().take(300).collect();
        let alignment = dna(&[&row]);
        let expected: Vec<usize> = (0..300).filter(|col| col % 3 != 0).collect();

        assert_eq!(
            alignment
                .grid(AlignmentType::Dna)
                .kept_columns(&[0], filter(Some(0.0), None)),
            expected
        );
    }

    #[test]
    fn kept_columns_reads_translated_grid() {
        let alignment = dna(&[b"ATGCCC", b"ATGAAA"]);
        assert_eq!(
            alignment
                .translated_grid(ReadingFrame::Frame1)
                .kept_columns(&[0, 1], filter(None, Some(1.0))),
            [1]
        );
    }
}
