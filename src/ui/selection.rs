use std::ops::Range;

use ratatui::layout::Rect;

use crate::core::CoreState;
use crate::ui::ui_state::MouseSelection;

#[must_use]
pub fn selection_point_crosshair(
    core: &CoreState,
    visible_rows: &[Option<usize>],
    sequence_rows_area: Rect,
    mouse_x: u16,
    mouse_y: u16,
) -> Option<(usize, usize)> {
    // Stops panic in debug mode when clicking outside the alignment pane sequence rows area.
    if !sequence_rows_area.contains((mouse_x, mouse_y).into()) {
        return None;
    }

    let row_index = usize::from(mouse_y - sequence_rows_area.y);
    let col_index = usize::from(mouse_x - sequence_rows_area.x);
    let sequence_id = visible_rows.get(row_index).copied().flatten()?;
    let absolute_col = core.viewport.window().col_range.start + col_index;
    // Limits selection in short alignments where the pane can extend beyond sequence length.
    (absolute_col < core.data().sequence_length).then_some((sequence_id, absolute_col))
}

/// Returns the (min, max) display-index bounds of a selection using the
/// pre-computed inverse map. Independent of which rows are currently visible
/// on screen, so the selection persists across scrolling.
#[must_use]
pub fn selection_row_bounds(selection: MouseSelection, display_index: &[usize]) -> (usize, usize) {
    let start = display_index[selection.sequence_id];
    let end = display_index[selection.end_sequence_id];
    (start.min(end), start.max(end))
}

#[must_use]
pub fn selection_visible_col_range(
    selection: MouseSelection,
    visible_col_range: &Range<usize>,
) -> Option<Range<usize>> {
    let col_min = selection.column.min(selection.end_column);
    let col_max = selection.column.max(selection.end_column);
    let visible_start = col_min.max(visible_col_range.start);
    let visible_end = col_max.saturating_add(1).min(visible_col_range.end);
    if visible_start < visible_end {
        Some(visible_start..visible_end)
    } else {
        None
    }
}
