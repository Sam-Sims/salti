use rayon::prelude::*;

use crate::Grid;

const PARALLEL_MIN_CELLS: usize = 1 << 16;
const MIN_ROWS_PER_JOB: usize = 128;
const MAX_BLOCK_COLUMNS: usize = 128;

pub(crate) fn count_blocks<'c>(
    grid: Grid<'_>,
    rows: &[usize],
    cols: &'c [usize],
) -> impl IndexedParallelIterator<Item = (&'c [usize], Vec<[u32; 128]>)> {
    cols.par_chunks(MAX_BLOCK_COLUMNS)
        .map(move |block| (block, count_rows(grid, rows, block)))
}

pub(crate) fn add_counts(total: &mut [u32; 128], other: &[u32; 128]) {
    for (total, other) in total.iter_mut().zip(other) {
        *total += other;
    }
}

struct ColumnCounts {
    pending: Vec<[u16; 128]>,
    totals: Vec<[u32; 128]>,
    pending_rows: u16,
}

impl ColumnCounts {
    fn new(width: usize) -> Self {
        Self {
            pending: vec![[0; 128]; width],
            totals: vec![[0; 128]; width],
            pending_rows: 0,
        }
    }

    #[inline]
    fn add_row(&mut self, bytes: impl IntoIterator<Item = u8>) {
        let mut counted = 0;
        for (table, byte) in self.pending.iter_mut().zip(bytes) {
            table[symbol_index(byte)] += 1;
            counted += 1;
        }
        debug_assert_eq!(counted, self.pending.len(), "one byte per column");
        self.pending_rows += 1;
        if self.pending_rows == u16::MAX {
            self.flush();
        }
    }

    fn into_totals(mut self) -> Vec<[u32; 128]> {
        self.flush();
        self.totals
    }

    fn flush(&mut self) {
        for (total, pending) in self.totals.iter_mut().zip(&mut self.pending) {
            for (total, pending) in total.iter_mut().zip(pending.iter_mut()) {
                *total += u32::from(std::mem::take(pending));
            }
        }
        self.pending_rows = 0;
    }
}

fn count_rows(grid: Grid<'_>, rows: &[usize], block: &[usize]) -> Vec<[u32; 128]> {
    let count_row = |mut counts: ColumnCounts, &row: &usize| {
        counts.add_row(grid.cells(row, block));
        counts
    };

    // only go parallel if we actually need it
    if rows.len().saturating_mul(block.len()) < PARALLEL_MIN_CELLS {
        return rows
            .iter()
            .fold(ColumnCounts::new(block.len()), count_row)
            .into_totals();
    }

    rows.par_iter()
        .with_min_len(MIN_ROWS_PER_JOB)
        .fold(|| ColumnCounts::new(block.len()), count_row)
        .map(ColumnCounts::into_totals)
        .reduce_with(merge_totals)
        .expect("rows should not be empty above min cells")
}

fn merge_totals(mut totals: Vec<[u32; 128]>, other: Vec<[u32; 128]>) -> Vec<[u32; 128]> {
    debug_assert_eq!(totals.len(), other.len());
    for (total, other) in totals.iter_mut().zip(&other) {
        add_counts(total, other);
    }
    totals
}

#[inline]
fn symbol_index(byte: u8) -> usize {
    debug_assert!(byte.is_ascii(), "sequence bytes are ASCII");
    usize::from(byte & 0x7f)
}

#[cfg(test)]
mod tests {
    use rayon::prelude::*;
    use rstest::rstest;

    use super::{ColumnCounts, add_counts, count_blocks, symbol_index};
    use crate::{Alignment, AlignmentType, Grid, RawSequence};

    const ROWS: usize = 600;
    const COLS: usize = 300;

    fn alignment() -> Alignment {
        Alignment::new_with_type(
            (0..ROWS).map(|row| RawSequence {
                id: format!("s{row}"),
                sequence: (0..COLS)
                    .map(|col| b"ACGT-"[(row * 7 + col * 13) % 5])
                    .collect(),
            }),
            AlignmentType::Dna,
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
        assert!(
            blocks
                .iter()
                .all(|block| block.len() <= super::MAX_BLOCK_COLUMNS)
        );
        assert_eq!(counts.concat(), expected_counts(grid, &rows, &cols));
    }

    #[test]
    fn add_row_counts_each_column() {
        let mut counts = ColumnCounts::new(3);
        counts.add_row(b"AC-".iter().copied());
        counts.add_row(b"AG-".iter().copied());

        let totals = counts.into_totals();
        assert_eq!(totals[0][symbol_index(b'A')], 2);
        assert_eq!(totals[1][symbol_index(b'C')], 1);
        assert_eq!(totals[1][symbol_index(b'G')], 1);
        assert_eq!(totals[2][symbol_index(b'-')], 2);
        assert_eq!(totals.iter().flatten().sum::<u32>(), 6);
    }

    #[test]
    fn add_row_flushes_before_u16_overflow() {
        let rows = 2 * u32::from(u16::MAX) + 10;
        let mut counts = ColumnCounts::new(1);
        for row in 0..rows {
            let byte = if row % 3 == 0 { b'-' } else { b'A' };
            counts.add_row([byte]);
        }

        let totals = counts.into_totals();
        assert_eq!(totals[0][symbol_index(b'-')], rows.div_ceil(3));
        assert_eq!(totals[0][symbol_index(b'A')], rows - rows.div_ceil(3));
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
