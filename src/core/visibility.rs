/// Manages visibility state for alignments, for both rows and columns.
///
/// Tracks which indices are hidden and provides mappings between visible and absolute indices for lookups.
#[derive(Debug, Clone, Default)]
pub struct Visibility {
    /// Vector of bools indicating whether each index is hidden (true) or visible (false).
    /// Will always be same length as number of absolute indices (e.g. total rows or columns).
    hidden: Vec<bool>,
    /// Maps each visible index to its absolute index.
    visible_to_absolute: Vec<usize>,
    /// Maps each absolute index to its visible index if visible.
    absolute_to_visible: Vec<Option<usize>>,
}

impl Visibility {
    /// Resets hidden sequences and rebuilds mappings.
    ///
    /// Use this when new data is loaded or axis dimensions change.
    pub fn reset_all(&mut self, len: usize) {
        self.hidden.clear();
        self.hidden.resize(len, false);
        self.reset_absolute_to_visible();
        self.visible_to_absolute.clear();
        self.visible_to_absolute.reserve(len);
        for absolute_index in 0..len {
            let visible_index = self.visible_to_absolute.len();
            self.visible_to_absolute.push(absolute_index);
            self.absolute_to_visible[absolute_index] = Some(visible_index);
        }
    }

    /// Returns number of currently visible indices.
    #[must_use]
    pub fn visible_count(&self) -> usize {
        self.visible_to_absolute.len()
    }

    #[must_use]
    pub fn visible_to_absolute(&self) -> &[usize] {
        &self.visible_to_absolute
    }

    /// Returns whether an absolute index is hidden.
    #[must_use]
    pub fn is_hidden(&self, absolute_index: usize) -> bool {
        self.hidden[absolute_index]
    }

    /// Updates hidden flags only; caller must rebuild mappings with `set_visible_order` before lookups.
    pub fn set_hidden<F>(&mut self, mut is_hidden: F)
    where
        F: FnMut(usize) -> bool,
    {
        for (absolute_index, hidden_flag) in self.hidden.iter_mut().enumerate() {
            *hidden_flag = is_hidden(absolute_index);
        }
    }

    /// Resets absolute to visible mapping
    fn reset_absolute_to_visible(&mut self) {
        self.absolute_to_visible.clear();
        self.absolute_to_visible.resize(self.hidden.len(), None);
    }

    /// Sets visible ordering when changed, e.g from pinned sequences.
    pub fn set_visible_order(&mut self, ordered_ids: &[usize]) {
        self.reset_absolute_to_visible();
        self.visible_to_absolute.clear();
        self.visible_to_absolute.reserve(ordered_ids.len());

        for &absolute_index in ordered_ids {
            let visible_index = self.visible_to_absolute.len();
            self.visible_to_absolute.push(absolute_index);
            self.absolute_to_visible[absolute_index] = Some(visible_index);
        }
    }
}
