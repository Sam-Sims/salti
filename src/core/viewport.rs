use std::ops::Range;

// this is essentially the scroll position in each axis
// i.e the top-left of the visible area
#[derive(Debug, Clone, Default)]
pub struct ViewportOffsets {
    pub rows: usize,
    pub cols: usize,
    pub names: usize,
}

// this is how much is visible on each axis.
// this is affected by terminal size/layout etc.
#[derive(Debug, Clone, Default)]
pub struct ViewportDims {
    pub rows: usize,
    pub cols: usize,
    pub name_width: usize,
}

// maximum bounds of the data.
// this is the full size of the alignment and name data, independent of the terminal.
#[derive(Debug, Clone, Default)]
pub struct ViewportMax {
    pub rows: usize,
    pub cols: usize,
    pub name_width: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Viewport {
    pub offsets: ViewportOffsets,
    pub dims: ViewportDims,
    pub max_size: ViewportMax,
}

#[derive(Debug, Clone)]
pub struct ViewportWindow {
    pub row_range: Range<usize>,
    pub col_range: Range<usize>,
    pub name_range: Range<usize>,
}

impl Viewport {
    pub fn set_dimensions(
        &mut self,
        visible_cols: usize,
        visible_rows: usize,
        name_visible_width: usize,
    ) {
        self.dims.cols = visible_cols;
        self.dims.rows = visible_rows;
        self.dims.name_width = name_visible_width;
    }

    pub fn update_dimensions(
        &mut self,
        visible_cols: usize,
        visible_rows: usize,
        name_visible_width: usize,
    ) {
        self.set_dimensions(visible_cols, visible_rows, name_visible_width);
        self.clamp_offsets();
    }

    #[must_use]
    pub fn window(&self) -> ViewportWindow {
        let row_end = self
            .offsets
            .rows
            .saturating_add(self.dims.rows)
            .min(self.max_size.rows);
        let col_end = self
            .offsets
            .cols
            .saturating_add(self.dims.cols)
            .min(self.max_size.cols);
        let name_end = self
            .offsets
            .names
            .saturating_add(self.dims.name_width)
            .min(self.max_size.name_width);

        ViewportWindow {
            row_range: self.offsets.rows..row_end,
            col_range: self.offsets.cols..col_end,
            name_range: self.offsets.names..name_end,
        }
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let max_scroll = self.max_size.rows.saturating_sub(self.dims.rows);
        self.offsets.rows = (self.offsets.rows + amount).min(max_scroll);
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.offsets.rows = self.offsets.rows.saturating_sub(amount);
    }

    pub fn scroll_right(&mut self, amount: usize) {
        let max_scroll = self.max_size.cols.saturating_sub(self.dims.cols);
        self.offsets.cols = (self.offsets.cols + amount).min(max_scroll);
    }

    pub fn scroll_left(&mut self, amount: usize) {
        self.offsets.cols = self.offsets.cols.saturating_sub(amount);
    }

    pub fn scroll_names_right(&mut self, amount: usize) {
        let max_scroll = self
            .max_size
            .name_width
            .saturating_sub(self.dims.name_width);
        self.offsets.names = (self.offsets.names + amount).min(max_scroll);
    }

    pub fn scroll_names_left(&mut self, amount: usize) {
        self.offsets.names = self.offsets.names.saturating_sub(amount);
    }

    pub fn jump_to_position(&mut self, position: usize) {
        let max_scroll = self.max_size.cols.saturating_sub(self.dims.cols);
        self.offsets.cols = position.min(max_scroll);
    }

    pub fn jump_to_sequence(&mut self, sequence_index: usize) {
        let max_scroll = self.max_size.rows.saturating_sub(self.dims.rows);
        self.offsets.rows = sequence_index.min(max_scroll);
    }

    pub fn clamp_offsets(&mut self) {
        let row_max = self.max_size.rows.saturating_sub(self.dims.rows);
        let col_max = self.max_size.cols.saturating_sub(self.dims.cols);
        let name_max_scroll = self
            .max_size
            .name_width
            .saturating_sub(self.dims.name_width);

        self.offsets.rows = self.offsets.rows.min(row_max);
        self.offsets.cols = self.offsets.cols.min(col_max);
        self.offsets.names = self.offsets.names.min(name_max_scroll);
    }
}
