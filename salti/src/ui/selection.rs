use std::ops::Range;

use ratatui::layout::Rect;

use crate::{
    core::{Viewport, model::AlignmentModel},
    ui::{layout::pinned_section_layout, ui_state::MouseSelection},
};

pub fn codon_span_for_absolute_column(
    absolute_col: usize,
    frame: libmsa::ReadingFrame,
    nucleotide_len: usize,
) -> Option<Range<usize>> {
    let offset = frame.offset();
    if absolute_col < offset {
        return None;
    }

    let codon_start = offset + ((absolute_col - offset) / 3) * 3;
    let codon_end = codon_start + 3;
    (codon_end <= nucleotide_len).then_some(codon_start..codon_end)
}

pub fn selection_point_crosshair(
    alignment: &AlignmentModel,
    viewport: &Viewport,
    sequence_rows_area: Rect,
    mouse_x: u16,
    mouse_y: u16,
) -> Option<(usize, usize)> {
    if !sequence_rows_area.contains((mouse_x, mouse_y).into()) {
        return None;
    }

    let row_offset = usize::from(mouse_y - sequence_rows_area.y);
    let col_offset = usize::from(mouse_x - sequence_rows_area.x);

    let window = viewport.window();
    let relative_col = window.col_range.start + col_offset;
    let absolute_col = alignment.view().absolute_column_id(relative_col)?;
    let band = pinned_section_layout(
        alignment.rows().pinned().len(),
        sequence_rows_area.height as usize,
    );

    let absolute_row = if row_offset < band.pinned_rendered {
        let pinned = alignment.rows().pinned();
        let pinned_index = row_offset;
        if pinned_index >= pinned.len() {
            return None;
        }
        pinned[pinned_index]
    } else if row_offset < band.pinned_rendered + band.divider_height {
        return None;
    } else {
        let scroll_offset = row_offset - band.pinned_rendered - band.divider_height;
        let relative_row = window.row_range.start + scroll_offset;
        alignment.view().absolute_row_id(relative_row)?
    };

    Some((absolute_row, absolute_col))
}

pub fn selection_row_bounds(selection: MouseSelection) -> (usize, usize) {
    let start = selection.sequence_id;
    let end = selection.end_sequence_id;
    (start.min(end), start.max(end))
}

pub fn selection_visible_col_range(
    selection: MouseSelection,
    alignment: &AlignmentModel,
    visible_col_range: &Range<usize>,
) -> Option<Range<usize>> {
    match alignment.translation() {
        Some(frame) => {
            translated_selection_visible_col_range(selection, alignment, visible_col_range, frame)
        }
        None => raw_selection_visible_col_range(selection, alignment, visible_col_range),
    }
}

fn raw_selection_visible_col_range(
    selection: MouseSelection,
    alignment: &AlignmentModel,
    visible_col_range: &Range<usize>,
) -> Option<Range<usize>> {
    let abs_start = selection.column.min(selection.end_column);
    let abs_end = selection.column.max(selection.end_column);
    let view = alignment.view();

    let mut rel_start: Option<usize> = None;
    let mut rel_end: Option<usize> = None;

    for rel in visible_col_range.clone() {
        if let Some(abs) = view.absolute_column_id(rel)
            && abs >= abs_start
            && abs <= abs_end
        {
            if rel_start.is_none() {
                rel_start = Some(rel);
            }
            rel_end = Some(rel + 1);
        }
    }

    rel_start.zip(rel_end).map(|(start, end)| start..end)
}

fn translated_selection_visible_col_range(
    selection: MouseSelection,
    alignment: &AlignmentModel,
    visible_col_range: &Range<usize>,
    frame: libmsa::ReadingFrame,
) -> Option<Range<usize>> {
    let selection_start = selection.column.min(selection.end_column);
    let selection_end = selection.column.max(selection.end_column) + 1;
    let view = alignment.view();
    let nucleotide_len = view.column_count();

    let mut rel_start: Option<usize> = None;
    let mut rel_end: Option<usize> = None;
    let mut previous_codon_start = None;

    for rel in visible_col_range.clone() {
        let Some(abs) = view.absolute_column_id(rel) else {
            continue;
        };
        let Some(codon_span) = codon_span_for_absolute_column(abs, frame, nucleotide_len) else {
            continue;
        };
        if previous_codon_start == Some(codon_span.start) {
            continue;
        }
        previous_codon_start = Some(codon_span.start);

        if codon_span.end <= selection_start || codon_span.start >= selection_end {
            continue;
        }

        let clipped_start = codon_span.start.max(visible_col_range.start);
        let clipped_end = codon_span.end.min(visible_col_range.end);
        let start = view
            .relative_column_id(clipped_start)
            .expect("translated mode requires a full visible column set");
        let end = view
            .relative_column_id(clipped_end - 1)
            .expect("translated mode requires a full visible column set")
            + 1;
        rel_start = Some(rel_start.map_or(start, |current| current.min(start)));
        rel_end = Some(rel_end.map_or(end, |current| current.max(end)));
    }

    rel_start.zip(rel_end).map(|(start, end)| start..end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Viewport;
    use crate::core::model::AlignmentModel;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn alignment_model(ids: &[&str]) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(ids.iter().map(|id| raw(id, b"ACGT")))
            .expect("test alignment should be valid");
        AlignmentModel::new(alignment).expect("base alignment should be accepted")
    }

    #[test]
    fn selection_row_bounds_normalises_order() {
        let selection = MouseSelection {
            sequence_id: 5,
            column: 0,
            end_sequence_id: 2,
            end_column: 3,
        };
        assert_eq!(selection_row_bounds(selection), (2, 5));

        let selection = MouseSelection {
            sequence_id: 1,
            column: 0,
            end_sequence_id: 4,
            end_column: 3,
        };
        assert_eq!(selection_row_bounds(selection), (1, 4));
    }

    #[test]
    fn selection_visible_col_range_maps_absolute_to_relative() {
        let model = alignment_model(&["s1", "s2"]);
        // Unfiltered: absolute == relative for columns.
        let selection = MouseSelection {
            sequence_id: 0,
            column: 1,
            end_sequence_id: 0,
            end_column: 2,
        };

        let range = selection_visible_col_range(selection, &model, &(0..4));
        assert_eq!(range, Some(1..3));
    }

    #[test]
    fn selection_visible_col_range_returns_none_when_outside_viewport() {
        let model = alignment_model(&["s1", "s2"]);
        let selection = MouseSelection {
            sequence_id: 0,
            column: 10,
            end_sequence_id: 0,
            end_column: 20,
        };

        let range = selection_visible_col_range(selection, &model, &(0..4));
        assert!(range.is_none());
    }

    #[test]
    fn codon_span_maps_any_column_in_the_same_codon() {
        let frame = libmsa::ReadingFrame::Frame1;

        assert_eq!(codon_span_for_absolute_column(0, frame, 9), Some(0..3));
        assert_eq!(codon_span_for_absolute_column(1, frame, 9), Some(0..3));
        assert_eq!(codon_span_for_absolute_column(2, frame, 9), Some(0..3));
        assert_eq!(codon_span_for_absolute_column(3, frame, 9), Some(3..6));
    }

    #[test]
    fn codon_span_returns_none_for_partial_frame_edges() {
        let frame = libmsa::ReadingFrame::Frame2;

        assert_eq!(codon_span_for_absolute_column(0, frame, 9), None);
        assert_eq!(codon_span_for_absolute_column(8, frame, 9), None);
    }

    #[test]
    fn translated_selection_visible_col_range_expands_to_overlapping_codon() {
        let alignment =
            libmsa::Alignment::new(vec![raw("s1", b"ATGAAATTT"), raw("s2", b"ATGAAATTT")])
                .expect("alignment should be valid");
        let mut model = AlignmentModel::new(alignment).expect("alignment model should be accepted");
        model
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .expect("translation should succeed");

        let selection = MouseSelection {
            sequence_id: 0,
            column: 1,
            end_sequence_id: 0,
            end_column: 1,
        };

        let range = selection_visible_col_range(selection, &model, &(0..9));
        assert_eq!(range, Some(0..3));
    }

    #[test]
    fn translated_selection_visible_col_range_clips_to_visible_window() {
        let alignment =
            libmsa::Alignment::new(vec![raw("s1", b"ATGAAATTT"), raw("s2", b"ATGAAATTT")])
                .expect("alignment should be valid");
        let mut model = AlignmentModel::new(alignment).expect("alignment model should be accepted");
        model
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .expect("translation should succeed");

        let selection = MouseSelection {
            sequence_id: 0,
            column: 1,
            end_sequence_id: 0,
            end_column: 1,
        };

        let range = selection_visible_col_range(selection, &model, &(1..5));
        assert_eq!(range, Some(1..3));
    }

    #[test]
    fn selection_visible_col_range_uses_absolute_columns_after_gap_filtering() {
        let alignment = libmsa::Alignment::new(vec![raw("s1", b"A--T"), raw("s2", b"A--T")])
            .expect("alignment should be valid");
        let mut model = AlignmentModel::new(alignment).expect("alignment model should be accepted");
        model
            .set_gap_filter(Some(0.0))
            .expect("gap filter should succeed");

        let selection = MouseSelection {
            sequence_id: 0,
            column: 0,
            end_sequence_id: 0,
            end_column: 3,
        };

        let range = selection_visible_col_range(selection, &model, &(0..2));
        assert_eq!(range, Some(0..2));
    }

    #[test]
    fn selection_point_crosshair_maps_to_absolute_filtered_column() {
        let alignment = libmsa::Alignment::new(vec![raw("s1", b"A--T"), raw("s2", b"A--T")])
            .expect("alignment should be valid");
        let mut model = AlignmentModel::new(alignment).expect("alignment model should be accepted");
        model
            .set_gap_filter(Some(0.0))
            .expect("gap filter should succeed");

        let mut viewport = Viewport::default();
        viewport.update_dimensions(2, 2, 2);
        viewport.set_bounds(2, 2, 2);

        let area = Rect::new(0, 0, 2, 2);
        let result = selection_point_crosshair(&model, &viewport, area, 1, 0);
        assert_eq!(result, Some((0, 3)));
    }

    #[test]
    fn crosshair_returns_none_outside_area() {
        let model = alignment_model(&["s1", "s2", "s3"]);
        let mut viewport = Viewport::default();
        viewport.update_dimensions(4, 3, 2);
        viewport.set_bounds(3, 4, 2);

        let area = Rect::new(10, 10, 4, 3);
        // Click outside area.
        assert!(selection_point_crosshair(&model, &viewport, area, 5, 5).is_none());
    }

    #[test]
    fn crosshair_maps_scroll_band_correctly() {
        let model = alignment_model(&["s1", "s2", "s3"]);
        let mut viewport = Viewport::default();
        viewport.update_dimensions(4, 3, 2);
        viewport.set_bounds(3, 4, 2);

        let area = Rect::new(0, 0, 4, 3);
        // No pinned rows, so row 0 maps to absolute row 0.
        let result = selection_point_crosshair(&model, &viewport, area, 0, 0);
        assert_eq!(result, Some((0, 0)));

        // Row 2, col 3.
        let result = selection_point_crosshair(&model, &viewport, area, 3, 2);
        assert_eq!(result, Some((2, 3)));
    }

    #[test]
    fn crosshair_handles_pinned_band() {
        let mut model = alignment_model(&["s1", "s2", "s3", "s4"]);
        model.pin(0).expect("should pin");
        // View now has rows [1, 2, 3] (row 0 excluded).
        // Pinned band: 1 row (row 0).
        // Divider: 1 row.
        // Scrollable: remaining rows.

        let mut viewport = Viewport::default();
        viewport.update_dimensions(4, 2, 2);
        viewport.set_bounds(3, 4, 2);

        let area = Rect::new(0, 0, 4, 4);
        // Row offset 0 -> pinned row 0 -> absolute row 0.
        let result = selection_point_crosshair(&model, &viewport, area, 0, 0);
        assert_eq!(result, Some((0, 0)));

        // Row offset 1 -> divider -> None.
        let result = selection_point_crosshair(&model, &viewport, area, 0, 1);
        assert!(result.is_none());

        // Row offset 2 -> scrollable row 0 -> view relative 0 -> absolute row 1.
        let result = selection_point_crosshair(&model, &viewport, area, 0, 2);
        assert_eq!(result, Some((1, 0)));
    }
}
