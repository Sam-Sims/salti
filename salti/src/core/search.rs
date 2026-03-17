use nucleo_matcher::Utf32Str;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Fuzzy,
    Regex,
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
}

impl SearchableList {
    pub fn new(filter_mode: FilterMode, leading_item: Option<&'static str>) -> Self {
        Self {
            items: Vec::new(),
            filtered_indices: Vec::new(),
            selection: None,
            leading_item: leading_item.map(ToString::to_string),
            filter_mode,
            query: String::new(),
            filter_error: false,
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
                let mut matcher = nucleo_matcher::Matcher::default();
                let pattern = Pattern::new(
                    &self.query,
                    CaseMatching::Ignore,
                    Normalization::Smart,
                    AtomKind::Fuzzy,
                );
                let mut utf32_buf = Vec::new();
                let mut scored: Vec<(u32, usize)> = self
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(index, name)| {
                        pattern
                            .score(Utf32Str::new(name, &mut utf32_buf), &mut matcher)
                            .map(|score| (score, index))
                    })
                    .collect();
                scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                self.filtered_indices = scored.into_iter().map(|(_, index)| index).collect();
            }
            FilterMode::Regex => {
                if let Ok(regex) = regex::Regex::new(&self.query) {
                    self.filtered_indices = self
                        .items
                        .iter()
                        .enumerate()
                        .filter(|(_, name)| regex.is_match(name))
                        .map(|(index, _)| index)
                        .collect();
                } else {
                    self.filtered_indices.clear();
                    self.filter_error = true;
                }
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

    pub fn move_selection_wrapped(&mut self, forwards: bool) {
        let total = self.visible_len();
        if total == 0 {
            self.selection = None;
            return;
        }

        let next = match (self.selection, forwards) {
            (None, true) => 0,
            (None, false) | (Some(0), false) => total - 1,
            (Some(selection), true) => (selection + 1) % total,
            (Some(selection), false) => selection - 1,
        };

        self.selection = Some(next);
    }

    #[must_use]
    pub fn selected_display_index(&self) -> Option<usize> {
        self.selection
    }

    #[must_use]
    pub fn selected_label(&self) -> Option<&str> {
        let selection = self.selection?;
        self.visible_item_at(selection)
    }

    #[must_use]
    pub fn visible_len(&self) -> usize {
        self.filtered_indices.len() + usize::from(self.leading_item.is_some())
    }

    #[must_use]
    pub fn visible_item_at(&self, display_index: usize) -> Option<&str> {
        if let Some(label) = self.leading_item.as_deref() {
            if display_index == 0 {
                return Some(label);
            }
            let item_index = *self.filtered_indices.get(display_index - 1)?;
            return self.items.get(item_index).map(String::as_str);
        }

        let item_index = *self.filtered_indices.get(display_index)?;
        self.items.get(item_index).map(String::as_str)
    }

    #[must_use]
    pub fn has_visible_items(&self) -> bool {
        self.visible_len() > 0
    }
}
