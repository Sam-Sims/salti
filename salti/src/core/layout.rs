use crate::core::session::ViewState;

#[derive(Debug)]
pub struct Layout {
    rows: Vec<usize>,
    pinned: usize,
    columns: Vec<usize>,
}

impl Layout {
    pub fn build(base_alignment: &libmsa::Alignment, state: &ViewState) -> Self {
        let (pinned, reference) = (state.pinned(), state.reference());
        debug_assert!(
            pinned
                .iter()
                .enumerate()
                .all(|(i, &row)| Some(row) != reference && !pinned[..i].contains(&row)),
            "pinned rows must be unique and not the reference"
        );

        let mut rows = pinned.to_vec();
        rows.extend((0..base_alignment.row_count()).filter(|&row| {
            Some(row) != reference
                && !pinned.contains(&row)
                && state
                    .row_regex_filter
                    .as_ref()
                    .is_none_or(|regex| regex.is_match(base_alignment.id(row)))
        }));

        let columns = state
            .grid(base_alignment)
            .kept_columns(&rows, state.mode().filter());

        Self {
            rows,
            pinned: pinned.len(),
            columns,
        }
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

    pub fn visible_column_position(&self, column: usize) -> Option<usize> {
        let position = self.column_position(column);
        (position < self.columns.len()).then_some(position)
    }

    /// Position in rows of `row` or the next visible row if its hidden.
    pub fn row_position(&self, row: usize) -> usize {
        self.main().partition_point(|&r| r < row)
    }
}
