use nucleo_matcher::{
    Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterMode {
    Fuzzy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Debug, Clone)]
pub struct SearchableList {
    items: Vec<String>,
    filtered_indices: Vec<usize>,
    selection: Option<usize>,
    leading_item: Option<String>,
    filter_mode: FilterMode,
    query: String,
    pub(crate) filter_error: bool,
    fuzzy_matcher: nucleo_matcher::Matcher,
    utf32_buf: Vec<char>,
}

impl SearchableList {
    pub fn new(filter_mode: FilterMode, leading_item: Option<String>) -> Self {
        Self {
            items: Vec::new(),
            filtered_indices: Vec::new(),
            selection: None,
            leading_item,
            filter_mode,
            query: String::new(),
            filter_error: false,
            fuzzy_matcher: nucleo_matcher::Matcher::default(),
            utf32_buf: Vec::new(),
        }
    }

    fn clamp_selection(&mut self) {
        let total = self.visible_len();
        if total == 0 {
            self.selection = None;
        } else if self.selection.is_some_and(|selection| selection >= total) {
            self.selection = Some(total - 1);
        }
    }

    fn apply_filter(&mut self) {
        self.filter_error = false;

        if self.query.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
            self.clamp_selection();
            return;
        }

        match self.filter_mode {
            FilterMode::Fuzzy => {
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
            }
        }

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
        self.filtered_indices.len() + usize::from(self.leading_item.is_some())
    }

    pub fn visible_item_at(&self, display_index: usize) -> Option<&str> {
        match (self.leading_item.as_deref(), display_index) {
            (Some(label), 0) => Some(label),
            (Some(_), _) => self.filtered_item_at(display_index - 1),
            (None, _) => self.filtered_item_at(display_index),
        }
    }

    fn filtered_item_at(&self, filtered_index: usize) -> Option<&str> {
        let item_index = *self.filtered_indices.get(filtered_index)?;
        self.items.get(item_index).map(String::as_str)
    }

    pub fn has_visible_items(&self) -> bool {
        self.visible_len() > 0
    }
}
