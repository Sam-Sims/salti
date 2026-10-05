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

    pub fn column_rank(&self, column: usize) -> usize {
        self.columns.partition_point(|&c| c < column)
    }

    pub fn row_rank(&self, row: usize) -> usize {
        self.main().partition_point(|&r| r < row)
    }
}
