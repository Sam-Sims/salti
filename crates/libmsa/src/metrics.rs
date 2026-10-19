use rayon::prelude::*;

use crate::{ColumnCounts, Grid};

const PARALLEL_MIN_CELLS: usize = 1 << 16;
const MIN_ROWS_PER_JOB: usize = 128;
const MAX_BLOCK_COLUMNS: usize = 128;

impl Grid<'_> {
    /// Counts each id in `cols` over `rows` and returns `per_column` of each, in `cols` order
    ///
    /// `per_column` turns one column's [`ColumnCounts`] and its id into a value, such as its
    /// [consensus](ColumnCounts::consensus) or [entropy](ColumnCounts::entropy), or a tuple of
    /// several. The id lets it read another cell of the column, such as the reference row's.
    ///
    /// Ids may be in any order and may repeat, and each occurrence is counted. With no
    /// `rows`, every column's counts are empty
    pub fn map_column_counts<T: Send>(
        self,
        rows: &[usize],
        cols: &[usize],
        per_column: impl Fn(ColumnCounts, usize) -> T + Sync,
    ) -> Vec<T> {
        let alignment_type = self.alignment_type();
        let per_column = &per_column;
        count_blocks(self, rows, cols)
            .flat_map_iter(move |(block, counts)| {
                block.iter().zip(counts).map(move |(&col, counts)| {
                    per_column(ColumnCounts::new(counts, alignment_type), col)
                })
            })
            .collect()
    }

    /// Returns the ids of the columns that pass `filter` over `rows`, in ascending order
    ///
    /// An inactive filter keeps every column without counting. With no `rows`, every column passes
    ///
    /// # Panics
    ///
    /// If an id in `rows` is not below the row count
    pub fn kept_columns(self, rows: &[usize], filter: ColumnFilter) -> Vec<usize> {
        let cols: Vec<usize> = (0..self.width()).collect();
        if !filter.is_active() {
            return cols;
        }

        self.map_column_counts(rows, &cols, |counts, col| {
            filter.keeps(&counts).then_some(col)
        })
        .into_iter()
        .flatten()
        .collect()
    }
}

/// Rules for hiding columns. `None` turns a rule off, and both fractions are in `0.0..=1.0`
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColumnFilter {
    /// Hides columns whose fraction of gaps is above this
    pub max_gap_fraction: Option<f64>,
    /// Hides columns whose most common residue makes up at least this fraction of the
    /// residues. Gaps, unknown symbols and ambiguity codes are left out, and DNA and protein
    /// ignore case
    pub min_const_fraction: Option<f64>,
}

impl ColumnFilter {
    /// Returns true if either rule is on
    pub fn is_active(self) -> bool {
        self.max_gap_fraction.is_some() || self.min_const_fraction.is_some()
    }

    fn keeps(self, counts: &ColumnCounts) -> bool {
        self.max_gap_fraction
            .is_none_or(|threshold| counts.gap_fraction() <= threshold)
            && self.min_const_fraction.is_none_or(|threshold| {
                counts
                    .constant_fraction()
                    .is_none_or(|fraction| fraction < threshold)
            })
    }
}

fn count_blocks<'c>(
    grid: Grid<'_>,
    rows: &[usize],
    cols: &'c [usize],
) -> impl IndexedParallelIterator<Item = (&'c [usize], Vec<[u32; 128]>)> {
    cols.par_chunks(MAX_BLOCK_COLUMNS)
        .map(move |block| (block, count_rows(grid, rows, block)))
}

fn count_rows(grid: Grid<'_>, rows: &[usize], block: &[usize]) -> Vec<[u32; 128]> {
    // a run of columns is read as a slice rather than id by id
    let run = block
        .windows(2)
        .all(|pair| pair[1] == pair[0] + 1)
        .then(|| block[0]..block[0] + block.len());
    let count_row = |mut counts: Vec<[u32; 128]>, &row: &usize| {
        match &run {
            Some(run) => add_cells(&mut counts, grid.cells_in(row, run.clone())),
            None => add_cells(&mut counts, grid.cells(row, block)),
        }
        counts
    };

    // only go parallel if we actually need it
    if rows.len().saturating_mul(block.len()) < PARALLEL_MIN_CELLS {
        return rows.iter().fold(vec![[0; 128]; block.len()], count_row);
    }

    rows.par_iter()
        .with_min_len(MIN_ROWS_PER_JOB)
        .fold(|| vec![[0; 128]; block.len()], count_row)
        .reduce_with(|mut totals, other| {
            debug_assert_eq!(totals.len(), other.len());
            for (total, other) in totals.iter_mut().zip(&other) {
                add_counts(total, other);
            }
            totals
        })
        .expect("rows should not be empty above min cells")
}

fn add_cells(counts: &mut [[u32; 128]], cells: impl Iterator<Item = u8>) {
    for (table, byte) in counts.iter_mut().zip(cells) {
        table[symbol_index(byte)] += 1;
    }
}

fn add_counts(total: &mut [u32; 128], other: &[u32; 128]) {
    for (total, other) in total.iter_mut().zip(other) {
        *total += other;
    }
}

#[inline]
fn symbol_index(byte: u8) -> usize {
    debug_assert!(byte.is_ascii(), "sequence bytes are ASCII");
    usize::from(byte & 0x7f)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use rayon::prelude::*;
    use rstest::rstest;

    use super::{ColumnFilter, MAX_BLOCK_COLUMNS, add_counts, count_blocks, symbol_index};
    use crate::{Alignment, AlignmentType, ConsensusMethod, Grid, ReadingFrame, Sequence};

    fn dna(sequences: &[&[u8]]) -> Alignment {
        Alignment::new(
            sequences
                .iter()
                .enumerate()
                .map(|(i, sequence)| Sequence {
                    id: format!("s{i}"),
                    residues: sequence.to_vec(),
                })
                .collect(),
        )
        .unwrap()
    }

    fn sample() -> Alignment {
        dna(&[b"ACGT", b"ACGA", b"TTTA", b"--T-"])
    }

    #[rstest]
    #[case::all(&[0, 1, 2], &[0, 1, 2, 3], b"ACGA")]
    #[case::subset_rows(&[2], &[0, 1, 2, 3], b"TTTA")]
    #[case::cols_unsorted_repeated(&[0, 1, 2], &[3, 0, 3], b"AAA")]
    #[case::rows_repeated(&[2, 2, 0], &[0], b"T")]
    fn map_column_counts_works(
        #[case] rows: &[usize],
        #[case] cols: &[usize],
        #[case] expected: &[u8],
    ) {
        let alignment = sample();
        let consensus =
            alignment
                .grid(AlignmentType::Dna)
                .map_column_counts(rows, cols, |counts, _| {
                    counts.consensus(ConsensusMethod::MajorityNonGap)
                });
        let expected: Vec<_> = expected.iter().copied().map(Some).collect();
        assert_eq!(consensus, expected);
    }

    #[test]
    fn map_column_counts_passes_column_ids() {
        let alignment = sample();
        let ids =
            alignment
                .grid(AlignmentType::Dna)
                .map_column_counts(&[0], &[3, 0, 3], |_, col| col);
        assert_eq!(ids, [3, 0, 3]);
    }

    #[test]
    fn map_column_counts_is_empty() {
        let alignment = sample();
        let ids = alignment
            .grid(AlignmentType::Dna)
            .map_column_counts(&[0, 1], &[], |_, col| col);
        assert_eq!(ids, []);
    }

    #[test]
    fn map_column_counts_without_rows_counts_nothing() {
        let alignment = sample();
        let consensus =
            alignment
                .grid(AlignmentType::Dna)
                .map_column_counts(&[], &[0, 1], |counts, _| {
                    counts.consensus(ConsensusMethod::Majority)
                });
        assert_eq!(consensus, [None, None]);
    }

    #[rstest]
    #[case::dna(AlignmentType::Dna, Some(1.0))]
    #[case::generic(AlignmentType::Generic, None)]
    fn map_column_counts_uses_grid_alignment_type(
        #[case] alignment_type: AlignmentType,
        #[case] expected: Option<f64>,
    ) {
        let alignment = dna(&[b"A", b"A"]);
        let conservation =
            alignment
                .grid(alignment_type)
                .map_column_counts(&[0, 1], &[0], |counts, _| counts.conservation());
        assert_eq!(conservation, [expected]);
    }

    #[test]
    fn map_column_counts_keeps_cols_order_across_blocks() {
        let row: Vec<u8> = b"ACGT".iter().copied().cycle().take(300).collect();
        let alignment = dna(&[&row]);
        let cols: Vec<usize> = (0..300).rev().collect();

        let consensus =
            alignment
                .grid(AlignmentType::Dna)
                .map_column_counts(&[0], &cols, |counts, _| {
                    counts.consensus(ConsensusMethod::Majority)
                });
        let expected: Vec<_> = cols.iter().map(|&col| Some(row[col])).collect();
        assert_eq!(consensus, expected);
    }

    #[test]
    fn map_column_counts_reads_translated_grid() {
        let alignment = dna(&[b"ATGCCC"]);
        let consensus = alignment
            .translated_grid(ReadingFrame::Frame1)
            .map_column_counts(&[0], &[1, 0], |counts, _| {
                counts.consensus(ConsensusMethod::Majority)
            });
        assert_eq!(consensus, [Some(b'P'), Some(b'M')]);
    }

    fn filter_sample() -> Alignment {
        dna(&[b"AAAANN", b"AACANN", b"A-GC-N", b"A-T-A-"])
    }

    fn filter(max_gap_fraction: Option<f64>, min_const_fraction: Option<f64>) -> ColumnFilter {
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
        let alignment = filter_sample();
        assert_eq!(
            alignment
                .grid(AlignmentType::Dna)
                .kept_columns(rows, filter),
            expected
        );
    }

    #[rstest]
    #[case::dna(AlignmentType::Dna, &[2, 3, 5])]
    #[case::generic(AlignmentType::Generic, &[1, 2, 3, 4, 5])]
    fn kept_columns_uses_grid_alignment_type(
        #[case] alignment_type: AlignmentType,
        #[case] expected: &[usize],
    ) {
        let alignment = filter_sample();
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

    const ROWS: usize = 600;
    const COLS: usize = 300;

    fn alignment() -> Alignment {
        Alignment::new(
            (0..ROWS)
                .map(|row| Sequence {
                    id: format!("s{row}"),
                    residues: (0..COLS)
                        .map(|col| b"ACGT-"[(row * 7 + col * 13) % 5])
                        .collect(),
                })
                .collect(),
        )
        .unwrap()
    }

    fn expected_counts(grid: Grid<'_>, rows: &[usize], cols: &[usize]) -> Vec<[u32; 128]> {
        cols.iter()
            .map(|&col| {
                let mut counts = [0; 128];
                for &row in rows {
                    counts[symbol_index(grid.cell(row, col))] += 1;
                }
                counts
            })
            .collect()
    }

    #[rstest]
    #[case::single_block((0..3).collect(), vec![2, 0, 2])]
    #[case::across_blocks((0..3).collect(), (0..COLS).rev().collect())]
    #[case::parallel_rows((0..ROWS).collect(), (0..200).collect())]
    #[case::repeated_rows((0..ROWS).chain([5, 5]).collect(), (0..200).collect())]
    #[case::no_rows(vec![], (0..200).collect())]
    #[case::no_cols((0..3).collect(), vec![])]
    fn count_blocks_works(#[case] rows: Vec<usize>, #[case] cols: Vec<usize>) {
        let alignment = alignment();
        let grid = alignment.grid(AlignmentType::Dna);

        let (blocks, counts): (Vec<_>, Vec<_>) = count_blocks(grid, &rows, &cols).unzip();

        assert_eq!(blocks.concat(), cols);
        assert!(blocks.iter().all(|block| block.len() <= MAX_BLOCK_COLUMNS));
        assert_eq!(counts.concat(), expected_counts(grid, &rows, &cols));
    }

    #[test]
    fn add_counts_works() {
        let mut total = [0; 128];
        total[symbol_index(b'A')] = 2;
        let mut other = [0; 128];
        other[symbol_index(b'A')] = 1;
        other[symbol_index(b'C')] = 3;

        add_counts(&mut total, &other);

        assert_eq!(total[symbol_index(b'A')], 3);
        assert_eq!(total[symbol_index(b'C')], 3);
        assert_eq!(total.iter().sum::<u32>(), 6);
    }
}
