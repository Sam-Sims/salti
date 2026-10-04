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

fn count_rows(grid: Grid<'_>, rows: &[usize], block: &[usize]) -> Vec<[u32; 128]> {
    let count_row = |mut counts: Vec<[u32; 128]>, &row: &usize| {
        for (table, byte) in counts.iter_mut().zip(grid.cells(row, block)) {
            table[symbol_index(byte)] += 1;
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

#[inline]
fn symbol_index(byte: u8) -> usize {
    debug_assert!(byte.is_ascii(), "sequence bytes are ASCII");
    usize::from(byte & 0x7f)
}

#[cfg(test)]
mod tests {
    use rayon::prelude::*;
    use rstest::rstest;

    use super::{add_counts, count_blocks, symbol_index};
    use crate::{Alignment, AlignmentType, Grid, Sequence};

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
        assert!(
            blocks
                .iter()
                .all(|block| block.len() <= super::MAX_BLOCK_COLUMNS)
        );
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
