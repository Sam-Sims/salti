use std::ops::Range;

use crate::core::CoreState;
use crate::ui::state::MouseSelection;
use crate::ui::utils::split_pinned_rows;
#[must_use]
pub fn visible_sequence_rows(core: &CoreState, row_capacity: usize) -> Vec<Option<usize>> {
    let window = core.viewport.window();
    let pinned_count = core.visible_pinned_sequences().take(row_capacity).count();
    let (pinned_rows, has_pins, unpinned_rows) = split_pinned_rows(row_capacity, pinned_count);
    let mut rows = Vec::with_capacity(row_capacity);

    rows.extend(
        core.visible_pinned_sequences()
            .take(pinned_rows)
            .map(|sequence| Some(sequence.sequence_id)),
    );

    if has_pins {
        rows.push(None);
    }

    rows.extend(
        core.visible_unpinned_sequences()
            .skip(window.row_range.start)
            .take(unpinned_rows)
            .map(|sequence| Some(sequence.sequence_id)),
    );

    rows
}

#[must_use]
pub fn display_index_by_sequence_id(core: &CoreState) -> Vec<usize> {
    let mut indices = vec![0; core.data.sequences.len()];
    for (display_index, sequence_id) in core.display_sequence_ids.iter().copied().enumerate() {
        indices[sequence_id] = display_index;
    }
    indices
}

#[must_use]
pub fn selection_row_bounds(
    selection: MouseSelection,
    display_index_by_sequence_id: &[usize],
) -> (usize, usize) {
    let start = display_index_by_sequence_id[selection.sequence_id];
    let end = display_index_by_sequence_id[selection.end_sequence_id];
    (start.min(end), start.max(end))
}

#[must_use]
pub fn selection_visible_col_range(
    selection: MouseSelection,
    visible_col_range: Range<usize>,
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
