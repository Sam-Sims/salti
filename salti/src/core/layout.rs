use anyhow::Result;

use crate::core::session::ViewState;

#[derive(Debug)]
pub struct Layout {
    rows: Vec<usize>,
    pinned: usize,
    columns: Vec<usize>,
}

impl Layout {
    pub fn build(base_alignment: &libmsa::Alignment, state: &ViewState) -> Result<Self> {
        todo!()
    }

    pub fn rows(&self) -> &[usize] {
        &self.rows
    }

    pub fn main(&self) -> &[usize] {
        &self.rows[self.pinned..]
    }

    pub fn pinned(&self) -> usize {
        self.pinned
    }

    pub fn columns(&self) -> &[usize] {
        &self.columns
    }

    /// Position in columns of `column` or the next visible column if its hidden.
    pub fn column_position(&self, column: usize) -> usize {
        self.columns.partition_point(|&c| c < column)
    }

    /// Position in rows of `row` or the next visible row if its hidden.
    pub fn row_position(&self, row: usize) -> usize {
        self.main().partition_point(|&r| r < row)
    }
}
