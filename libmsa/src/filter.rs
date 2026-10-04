use crate::Grid;

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
        todo!()
    }
}
