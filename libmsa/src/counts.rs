use rayon::{prelude::*};

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

fn count_rows(grid: Grid<'_>, rows: &[usize], block: &[usize]) -> Vec<[u32; 128]> {
    todo!()
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

#[inline]
fn symbol_index(byte: u8) -> usize {
    debug_assert!(byte.is_ascii(), "sequence bytes are ASCII");
    usize::from(byte & 0x7f)
}

fn merge_totals(mut totals: Vec<[u32; 128]>, other: Vec<[u32; 128]>) -> Vec<[u32; 128]> {
    debug_assert_eq!(totals.len(), other.len());
    for (total, other) in totals.iter_mut().zip(&other) {
        for (total, other) in total.iter_mut().zip(other) {
            *total += other;
        }
    }
    totals
}

#[cfg(test)]
mod tests {
    use super::{ColumnCounts, merge_totals, symbol_index};

    #[test]
    fn counts_each_column_separately() {
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
    fn flushes_before_u16_overflow() {
        let rows = 2 * usize::from(u16::MAX) + 10;
        let mut counts = ColumnCounts::new(1);
        for row in 0..rows {
            let byte = if row % 3 == 0 { b'-' } else { b'A' };
            counts.add_row([byte]);
        }

        let totals = counts.into_totals();
        assert_eq!(totals[0][symbol_index(b'-')], rows.div_ceil(3) as u32);
        assert_eq!(
            totals[0][symbol_index(b'A')],
            (rows - rows.div_ceil(3)) as u32
        );
    }

    #[test]
    fn merge_adds_totals() {
        let mut left = ColumnCounts::new(2);
        left.add_row(b"AC".iter().copied());
        let mut right = ColumnCounts::new(2);
        right.add_row(b"AT".iter().copied());

        let totals = merge_totals(left.into_totals(), right.into_totals());
        assert_eq!(totals[0][symbol_index(b'A')], 2);
        assert_eq!(totals[1][symbol_index(b'C')], 1);
        assert_eq!(totals[1][symbol_index(b'T')], 1);
    }
}
