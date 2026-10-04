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
