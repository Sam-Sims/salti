use nucleo_matcher::{
    Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Default)]
pub struct SearchableList {
    items: Vec<String>,
    filtered_indices: Vec<usize>,
    selection: Option<usize>,
    query: String,
    fuzzy_matcher: nucleo_matcher::Matcher,
    utf32_buf: Vec<char>,
}

impl SearchableList {
    fn clamp_selection(&mut self) {
        let total = self.visible_len();
        if total == 0 {
            self.selection = None;
        } else if self.selection.is_some_and(|selection| selection >= total) {
            self.selection = Some(total - 1);
        }
    }

    fn apply_filter(&mut self) {
        if self.query.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
            self.clamp_selection();
            return;
        }

        let pattern = Pattern::new(
            &self.query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let fuzzy_matcher = &mut self.fuzzy_matcher;
        let utf32_buf = &mut self.utf32_buf;
        let mut scored: Vec<(u32, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, name)| {
                pattern
                    .score(Utf32Str::new(name, utf32_buf), fuzzy_matcher)
                    .map(|score| (score, index))
            })
            .collect();
        scored.sort_unstable_by(|(score_a, idx_a), (score_b, idx_b)| {
            score_b.cmp(score_a).then_with(|| idx_a.cmp(idx_b))
        });
        self.filtered_indices = scored.into_iter().map(|(_, index)| index).collect();

        self.clamp_selection();
    }

    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.apply_filter();
    }

    pub fn set_items_and_query(&mut self, items: Vec<String>, query: &str) {
        self.items = items;
        query.clone_into(&mut self.query);
        self.apply_filter();
    }

    pub fn update_query(&mut self, query: &str) {
        if self.query == query {
            return;
        }

        query.clone_into(&mut self.query);
        self.apply_filter();
    }

    pub fn reset_selection(&mut self) {
        self.selection = None;
    }

    pub fn move_selection_wrapped(&mut self, direction: Direction) {
        let total = self.visible_len();
        if total == 0 {
            self.selection = None;
            return;
        }

        let next = match (self.selection, direction) {
            (None, Direction::Forward) => 0,
            (None | Some(0), Direction::Backward) => total - 1,
            (Some(selection), Direction::Forward) => (selection + 1) % total,
            (Some(selection), Direction::Backward) => selection - 1,
        };

        self.selection = Some(next);
    }

    pub fn selected_display_index(&self) -> Option<usize> {
        self.selection
    }

    pub fn selected_label(&self) -> Option<&str> {
        let selection = self.selection?;
        self.visible_item_at(selection)
    }

    pub fn visible_len(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn visible_item_at(&self, display_index: usize) -> Option<&str> {
        let item_index = *self.filtered_indices.get(display_index)?;
        self.items.get(item_index).map(String::as_str)
    }

    pub fn has_visible_items(&self) -> bool {
        self.visible_len() > 0
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn list(items: &[&str]) -> SearchableList {
        let mut list = SearchableList::default();
        list.set_items(items.iter().map(ToString::to_string).collect());
        list
    }

    fn visible(list: &SearchableList) -> Vec<&str> {
        (0..list.visible_len())
            .map(|index| list.visible_item_at(index).unwrap())
            .collect()
    }

    #[rstest]
    #[case::empty_query_keeps_order("", &["xaxbxc", "abc", "pin-b", "pin-a"])]
    #[case::best_match_first("abc", &["abc", "xaxbxc"])]
    #[case::ties_keep_item_order("pin", &["pin-b", "pin-a"])]
    #[case::ignores_case("ABC", &["abc", "xaxbxc"])]
    fn update_query_works(#[case] query: &str, #[case] expected: &[&str]) {
        let mut list = list(&["xaxbxc", "abc", "pin-b", "pin-a"]);

        list.update_query(query);

        assert_eq!(visible(&list), expected);
    }

    #[test]
    fn update_query_is_empty_without_match() {
        let mut list = list(&["abc"]);

        list.update_query("zzz");

        assert_eq!(visible(&list), [""; 0]);
    }

    #[rstest]
    #[case::first_forward(&[Direction::Forward], "a")]
    #[case::first_backward_is_last(&[Direction::Backward], "c")]
    #[case::next(&[Direction::Forward, Direction::Forward], "b")]
    #[case::previous(&[Direction::Backward, Direction::Backward], "b")]
    #[case::wraps_forward(&[Direction::Backward, Direction::Forward], "a")]
    #[case::wraps_backward(&[Direction::Forward, Direction::Backward], "c")]
    fn move_selection_wrapped_works(#[case] moves: &[Direction], #[case] expected: &str) {
        let mut list = list(&["a", "b", "c"]);

        for &direction in moves {
            list.move_selection_wrapped(direction);
        }

        assert_eq!(list.selected_label(), Some(expected));
    }

    #[test]
    fn move_selection_wrapped_rejects_empty_list() {
        let mut list = list(&[]);

        list.move_selection_wrapped(Direction::Forward);

        assert_eq!(list.selected_display_index(), None);
    }

    #[rstest]
    #[case::clamped_to_last("ab", Some("abc"))]
    #[case::cleared_without_match("zzz", None)]
    fn set_items_and_query_clamps_selection(#[case] query: &str, #[case] expected: Option<&str>) {
        let mut list = list(&["abc", "xyz", "abdc"]);
        list.move_selection_wrapped(Direction::Backward);

        list.set_items_and_query(vec!["abc".to_string(), "xyz".to_string()], query);

        assert_eq!(list.selected_label(), expected);
    }
}
