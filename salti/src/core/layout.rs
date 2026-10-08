use anyhow::{Result, bail};

use crate::core::session::{ViewMode, ViewState};

#[derive(Debug)]
pub struct Layout {
    rows: Vec<usize>,
    pinned: usize,
    columns: Vec<usize>,
}

impl Layout {
    pub fn build(base_alignment: &libmsa::Alignment, state: &ViewState) -> Result<Self> {
        debug_assert!(
            state.pinned.iter().enumerate().all(|(i, &row)| {
                Some(row) != state.reference && !state.pinned[..i].contains(&row)
            }),
            "pinned rows must be unique and not the reference"
        );
        if state.mode != ViewMode::Default && !state.alignment_type.supports_translation() {
            bail!(
                "Translation needs a DNA alignment. Use set-sequence-type if the type was detected wrongly"
            );
        }
        if state.mode == ViewMode::QuickTranslate && state.filter.is_active() {
            bail!(
                "Column filters can't be combined with the translation overlay. Turn the overlay off with t, or run clear-all-filters first"
            );
        }

        let mut rows = state.pinned.clone();
        rows.extend((0..base_alignment.row_count()).filter(|&row| {
            Some(row) != state.reference
                && !state.pinned.contains(&row)
                && state
                    .row_regex_filter
                    .as_ref()
                    .is_none_or(|regex| regex.is_match(base_alignment.id(row)))
        }));

        let columns = state.grid(base_alignment).kept_columns(&rows, state.filter);
        if columns.is_empty() {
            bail!("No columns would remain visible. Loosen the gap or constant filter");
        }

        Ok(Self {
            rows,
            pinned: state.pinned.len(),
            columns,
        })
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
