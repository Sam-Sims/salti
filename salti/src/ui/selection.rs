use std::ops::Range;

use ratatui::layout::Rect;

use crate::core::CoreState;
use crate::ui::ui_state::MouseSelection;

#[must_use]
pub fn selection_point_crosshair(
    core: &CoreState,
    visible_rows: &[Option<usize>],
    visible_col_start: usize,
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
    let visible_col = visible_col_start + col_index;
    let absolute_col = *core
        .column_visibility
        .visible_to_absolute
        .get(visible_col)?;
    Some((sequence_id, absolute_col))
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
    core: &CoreState,
    visible_col_range: &Range<usize>,
) -> Option<Range<usize>> {
    let absolute_start = selection.column.min(selection.end_column);
    let absolute_end = selection.column.max(selection.end_column);

    let (Some(visible_start), Some(visible_end_inclusive)) = (
        core.column_visibility
            .absolute_to_visible
            .get(absolute_start)
            .copied()
            .flatten(),
        core.column_visibility
            .absolute_to_visible
            .get(absolute_end)
            .copied()
            .flatten(),
    ) else {
        return None;
    };

    let start = visible_start
        .min(visible_end_inclusive)
        .max(visible_col_range.start);
    let end = visible_start
        .max(visible_end_inclusive)
        .saturating_add(1)
        .min(visible_col_range.end);

    (start < end).then_some(start..end)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::cli::StartupState;
    use crate::core::parser::Alignment;

    fn test_core() -> CoreState {
        let alignments = vec![Alignment {
            id: Arc::from("seq-a"),
            sequence: Arc::from(b"ACGT".to_vec()),
        }];
        let mut core = CoreState::new(StartupState::default());
        core.handle_alignments_loaded(Ok(alignments));
        core
    }

    #[test]
    fn selection_point_crosshair_maps_visible_x_to_absolute_column() {
        let mut core = test_core();
        core.column_visibility
            .set_hidden(|absolute_index| absolute_index == 1);
        core.update_viewport_dimensions(3, 1, 8);

        let visible_rows = [Some(0)];
        let area = Rect::new(0, 0, 3, 1);

        assert_eq!(
            selection_point_crosshair(&core, &visible_rows, core.viewport.offsets.cols, area, 1, 0,),
            Some((0, 2))
        );
    }
    #[test]
    fn selection_visible_col_range_maps_absolute_selection_to_visible_window_span() {
        let mut core = test_core();
        core.column_visibility
            .set_hidden(|absolute_index| absolute_index == 1);
        core.update_viewport_dimensions(3, 1, 8);

        let selection = MouseSelection {
            sequence_id: 0,
            column: 0,
            end_sequence_id: 0,
            end_column: 3,
        };

        let visible_range = core.viewport.window().col_range;
        assert_eq!(
            selection_visible_col_range(selection, &core, &visible_range),
            Some(0..3)
        );
    }
}
