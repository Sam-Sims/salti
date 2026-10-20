use super::ViewState;

/// which rows and columns are on screen, in the order they're drawn
#[derive(Debug)]
pub struct Layout {
    rows: Vec<usize>,
    pinned_count: usize,
    columns: Vec<usize>,
}

impl Layout {
    pub fn new(base_alignment: &libmsa::Alignment, state: &ViewState) -> Self {
        let (pinned, reference) = (state.pinned(), state.reference());

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
            pinned_count: pinned.len(),
            columns,
        }
    }

    /// every row on screen, top to bottom
    /// pinned rows come first, then the main rows
    pub fn rows(&self) -> &[usize] {
        &self.rows
    }

    /// the rows below the pinned ones, which the row filter applies to
    pub fn main(&self) -> &[usize] {
        &self.rows[self.pinned_count..]
    }

    pub fn pinned_count(&self) -> usize {
        self.pinned_count
    }

    /// the alignment column behind each column on screen, left to right
    /// in the protein view these are protein columns
    pub fn columns(&self) -> &[usize] {
        &self.columns
    }

    /// where `column` sits among the shown columns
    /// a hidden column gives the position of the next shown one
    pub fn column_position(&self, column: usize) -> usize {
        self.columns.partition_point(|&shown| shown < column)
    }

    /// like `column_position`, but `None` when no shown column is at or after `column`
    pub fn visible_column_position(&self, column: usize) -> Option<usize> {
        let position = self.column_position(column);
        (position < self.columns.len()).then_some(position)
    }

    /// where `row` sits among the main rows, below the pinned ones
    /// a row that isn't shown gives the position of the next main row
    pub fn row_position(&self, row: usize) -> usize {
        self.main().partition_point(|&main_row| main_row < row)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_utils::session;

    fn layout(rows: &[usize], pinned_count: usize, columns: &[usize]) -> Layout {
        Layout {
            rows: rows.to_vec(),
            pinned_count,
            columns: columns.to_vec(),
        }
    }

    #[rstest]
    #[case::all_rows(&[], None, None, &[0, 1, 2, 3], 0)]
    #[case::pinned_first_in_pin_order(&[3, 1], None, None, &[3, 1, 0, 2], 2)]
    #[case::reference_removed(&[], Some(1), None, &[0, 2, 3], 0)]
    #[case::regex_filters_main(&[], None, Some("s[13]"), &[1, 3], 0)]
    #[case::pinned_kept_when_regex_excludes(&[0], None, Some("s[13]"), &[0, 1, 3], 1)]
    fn new_rows_works(
        #[case] pinned: &[usize],
        #[case] reference: Option<usize>,
        #[case] regex: Option<&str>,
        #[case] expected_rows: &[usize],
        #[case] expected_pinned_count: usize,
    ) {
        let session = session(&[b"A", b"A", b"A", b"A"]);
        let mut state = session.state().clone();
        for &row in pinned {
            state.pin(row);
        }
        state.set_reference(reference);
        state.row_regex_filter = regex.map(|regex| regex::Regex::new(regex).unwrap());

        let layout = Layout::new(session.base_alignment(), &state);

        assert_eq!(layout.rows(), expected_rows);
        assert_eq!(layout.pinned_count(), expected_pinned_count);
    }

    #[rstest]
    #[case::no_pins(&[])]
    #[case::pinned(&[1])]
    fn new_main_is_empty(#[case] pinned: &[usize]) {
        let session = session(&[b"A", b"A"]);
        let mut state = session.state().clone();
        for &row in pinned {
            state.pin(row);
        }
        state.row_regex_filter = Some(regex::Regex::new("nothing").unwrap());

        let layout = Layout::new(session.base_alignment(), &state);

        assert_eq!(layout.rows(), pinned);
        assert_eq!(layout.pinned_count(), pinned.len());
    }

    #[test]
    fn new_columns_counts_pinned_rows() {
        let session = session(&[b"A-", b"AC", b"AC"]);
        let mut state = session.state().clone();
        state.pin(0);
        state.row_regex_filter = Some(regex::Regex::new("s1").unwrap());
        state.set_gap_filter(Some(0.4)).unwrap();

        let layout = Layout::new(session.base_alignment(), &state);

        assert_eq!(layout.columns(), [0]);
    }

    #[test]
    fn new_columns_filters_protein_columns_in_protein_view() {
        let session = session(&[b"ATGAAA", b"ATGAGA"]);
        let mut state = session.state().clone();
        state.toggle_protein_view().unwrap();
        state.set_constant_filter(Some(1.0)).unwrap();

        let layout = Layout::new(session.base_alignment(), &state);

        assert_eq!(layout.columns(), [1]);
    }

    #[rstest]
    #[case::below(0, 0)]
    #[case::at(4, 1)]
    #[case::between(5, 2)]
    #[case::past(9, 3)]
    fn column_position_works(#[case] column: usize, #[case] expected: usize) {
        assert_eq!(layout(&[], 0, &[2, 4, 6]).column_position(column), expected);
    }

    #[rstest]
    #[case::at(4, 1)]
    #[case::between(3, 1)]
    fn visible_column_position_works(#[case] column: usize, #[case] expected: usize) {
        assert_eq!(
            layout(&[], 0, &[2, 4, 6]).visible_column_position(column),
            Some(expected)
        );
    }

    #[test]
    fn visible_column_position_rejects_past_last_column() {
        assert_eq!(layout(&[], 0, &[2, 4, 6]).visible_column_position(7), None);
    }

    #[rstest]
    #[case::below(0, 0)]
    #[case::at(3, 1)]
    #[case::between(4, 2)]
    #[case::past(9, 3)]
    fn row_position_works(#[case] row: usize, #[case] expected: usize) {
        assert_eq!(layout(&[8, 1, 3, 5], 1, &[]).row_position(row), expected);
    }
}
