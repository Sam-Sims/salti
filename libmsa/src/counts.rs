use std::ops::Range;

use rayon::prelude::*;

use crate::{data::AlignmentData, projection::Projection};

const PARALLEL_MIN_CELLS: usize = 1 << 16;
const MIN_ROWS_PER_JOB: usize = 128;
const MAX_BLOCK_COLUMNS: usize = 128;
const BLOCKS_PER_THREAD: usize = 4;

#[inline]
fn symbol_index(byte: u8) -> usize {
    debug_assert!(byte.is_ascii(), "sequence bytes are ASCII");
    usize::from(byte & 0x7f)
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

pub(crate) fn count_columns(
    data: &AlignmentData,
    rows: &Projection,
    columns: &Projection,
    range: Range<usize>,
) -> Vec<[u32; 128]> {
    let width = range.len();
    if let Some(abs_cols) = columns.all_absolute_range(range.clone()) {
        return count_rows(rows, width, count_contiguous_row(data, abs_cols));
    }

    let abs_cols: Vec<usize> = range
        .map(|rel_col| {
            columns
                .absolute(rel_col)
                .expect("range lies within columns")
        })
        .collect();
    count_rows(rows, width, |counts, abs_row| {
        let sequence = &data.sequences[abs_row].sequence;
        counts.add_row(abs_cols.iter().map(|&abs_col| sequence[abs_col]));
    })
}

pub(crate) fn count_all_columns<'a>(
    data: &'a AlignmentData,
    rows: &'a Projection,
) -> impl IndexedParallelIterator<Item = (Range<usize>, Vec<[u32; 128]>)> + 'a {
    let block_width = data
        .length
        .div_ceil(rayon::current_num_threads() * BLOCKS_PER_THREAD)
        .clamp(1, MAX_BLOCK_COLUMNS);

    (0..data.length.div_ceil(block_width))
        .into_par_iter()
        .map(move |block| {
            let start = block * block_width;
            let abs_cols = start..(start + block_width).min(data.length);
            let counts = count_rows_on_this_thread(
                rows,
                abs_cols.len(),
                count_contiguous_row(data, abs_cols.clone()),
            );
            (abs_cols, counts)
        })
}

fn count_contiguous_row(
    data: &AlignmentData,
    abs_cols: Range<usize>,
) -> impl Fn(&mut ColumnCounts, usize) + Sync + '_ {
    move |counts, abs_row| {
        let sequence = &data.sequences[abs_row].sequence;
        counts.add_row(sequence[abs_cols.clone()].iter().copied());
    }
}

fn count_rows(
    rows: &Projection,
    width: usize,
    count_row: impl Fn(&mut ColumnCounts, usize) + Sync,
) -> Vec<[u32; 128]> {
    if rows.len().saturating_mul(width) < PARALLEL_MIN_CELLS {
        return count_rows_on_this_thread(rows, width, count_row);
    }

    (0..rows.len())
        .into_par_iter()
        .with_min_len(MIN_ROWS_PER_JOB)
        .fold(
            || ColumnCounts::new(width),
            |mut counts, rel_row| {
                let abs_row = rows.absolute(rel_row).expect("relative row is in bounds");
                count_row(&mut counts, abs_row);
                counts
            },
        )
        .map(ColumnCounts::into_totals)
        .reduce_with(merge_totals)
        .expect("parallel counting runs at least one job")
}

fn count_rows_on_this_thread(
    rows: &Projection,
    width: usize,
    count_row: impl Fn(&mut ColumnCounts, usize),
) -> Vec<[u32; 128]> {
    let mut counts = ColumnCounts::new(width);
    for abs_row in rows.iter() {
        count_row(&mut counts, abs_row);
    }
    counts.into_totals()
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
