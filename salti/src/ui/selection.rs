use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color::Rgb;
use ratatui::widgets::Block;

use crate::{
    core::{Viewport, codon::TranslationOverlay, model::AlignmentModel},
    ui::{
        layout::{AppLayout, RULER_HEIGHT_ROWS, pinned_section_layout},
        ui_state::{MouseSelection, UiState},
    },
};

const SELECTION_ROW_HIGHLIGHT_ALPHA: f32 = 0.3;
const SELECTION_ROW_TINT_ALPHA: f32 = 0.22;
const SELECTION_COL_HIGHLIGHT_ALPHA: f32 = 0.28;

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

pub fn render_mouse_selection(
    f: &mut Frame,
    layout: &AppLayout,
    alignment: &AlignmentModel,
    ui: &UiState,
    viewport: &Viewport,
) {
    let Some(selection) = ui.selection else {
        return;
    };

    let window = viewport.window();
    let id_inner_area = Block::bordered().inner(layout.sequence_id_pane);
    let sequence_rows_area = layout.alignment_pane_sequence_rows;
    let id_content_y = id_inner_area.y + RULER_HEIGHT_ROWS;
    let id_end_x = id_inner_area.x.saturating_add(id_inner_area.width);
    let sequence_end_x = sequence_rows_area
        .x
        .saturating_add(sequence_rows_area.width);
    let band_layout = pinned_section_layout(
        alignment.rows().pinned().len(),
        sequence_rows_area.height as usize,
    );
    let (row_min, row_max) = selection_row_bounds(selection);

    for (row_offset, &absolute_row) in alignment
        .rows()
        .pinned()
        .iter()
        .take(band_layout.pinned_rendered)
        .enumerate()
    {
        if !(row_min..=row_max).contains(&absolute_row) {
            continue;
        }

        let row_y = sequence_rows_area.y + row_offset as u16;
        shader(
            f,
            id_inner_area,
            Rect::new(
                id_inner_area.x,
                id_content_y + row_offset as u16,
                id_end_x.saturating_sub(id_inner_area.x),
                1,
            ),
            ui.theme.theme.accent,
            SELECTION_ROW_HIGHLIGHT_ALPHA,
        );
        shader(
            f,
            sequence_rows_area,
            Rect::new(
                sequence_rows_area.x,
                row_y,
                sequence_end_x.saturating_sub(sequence_rows_area.x),
                1,
            ),
            ui.theme.theme.surface_bg,
            SELECTION_ROW_TINT_ALPHA,
        );
    }

    let scroll_start_y = sequence_rows_area.y
        + band_layout.pinned_rendered as u16
        + band_layout.divider_height as u16;
    for (row_offset, relative_row) in window.row_range.clone().enumerate() {
        let Some(absolute_row) = alignment.view().absolute_row_id(relative_row) else {
            continue;
        };
        if !(row_min..=row_max).contains(&absolute_row) {
            continue;
        }

        let row_y = scroll_start_y + row_offset as u16;
        shader(
            f,
            id_inner_area,
            Rect::new(
                id_inner_area.x,
                id_content_y
                    + band_layout.pinned_rendered as u16
                    + band_layout.divider_height as u16
                    + row_offset as u16,
                id_end_x.saturating_sub(id_inner_area.x),
                1,
            ),
            ui.theme.theme.accent,
            SELECTION_ROW_HIGHLIGHT_ALPHA,
        );
        shader(
            f,
            sequence_rows_area,
            Rect::new(
                sequence_rows_area.x,
                row_y,
                sequence_end_x.saturating_sub(sequence_rows_area.x),
                1,
            ),
            ui.theme.theme.surface_bg,
            SELECTION_ROW_TINT_ALPHA,
        );
    }

    if let Some(visible_col_range) =
        selection_visible_col_range(selection, alignment, &window.col_range)
    {
        let start_x =
            sequence_rows_area.x + (visible_col_range.start - window.col_range.start) as u16;
        let end_x_exclusive =
            sequence_rows_area.x + (visible_col_range.end - window.col_range.start) as u16;
        shader(
            f,
            sequence_rows_area,
            Rect::new(
                start_x,
                sequence_rows_area.y,
                end_x_exclusive.saturating_sub(start_x),
                sequence_rows_area.height,
            ),
            ui.theme.theme.panel_bg,
            SELECTION_COL_HIGHLIGHT_ALPHA,
        );
    }
}

pub fn selection_visible_col_range(
    selection: MouseSelection,
    alignment: &AlignmentModel,
    visible_col_range: &Range<usize>,
) -> Option<Range<usize>> {
    match alignment.translation_overlay() {
        Some(overlay) => translated_selection_visible_col_range(
            selection,
            alignment,
            visible_col_range,
            &overlay,
        ),
        None => raw_selection_visible_col_range(selection, alignment, visible_col_range),
    }
}

fn interpolate(from: u8, to: u8, alpha: f32) -> u8 {
    let from = f32::from(from);
    let to = f32::from(to);
    (from + (to - from) * alpha).round().clamp(0.0, 255.0) as u8
}

fn blend_background(
    base: ratatui::style::Color,
    tint: ratatui::style::Color,
    alpha: f32,
) -> ratatui::style::Color {
    match (base, tint) {
        (Rgb(red, green, blue), Rgb(red_tint, green_tint, blue_tint)) => Rgb(
            interpolate(red, red_tint, alpha),
            interpolate(green, green_tint, alpha),
            interpolate(blue, blue_tint, alpha),
        ),
        _ => tint,
    }
}

fn shader(
    f: &mut Frame,
    clip_area: Rect,
    tint_area: Rect,
    tint: ratatui::style::Color,
    alpha: f32,
) {
    if alpha <= 0.0 || clip_area.width == 0 || clip_area.height == 0 {
        return;
    }

    let x_start = tint_area.x.max(clip_area.x);
    let x_end = tint_area
        .x
        .saturating_add(tint_area.width)
        .min(clip_area.x.saturating_add(clip_area.width));
    let y_start = tint_area.y.max(clip_area.y);
    let y_end = tint_area
        .y
        .saturating_add(tint_area.height)
        .min(clip_area.y.saturating_add(clip_area.height));
    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let buffer = f.buffer_mut();
    for y in y_start..y_end {
        for x in x_start..x_end {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_bg(blend_background(cell.bg, tint, alpha));
            }
        }
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
    overlay: &TranslationOverlay,
) -> Option<Range<usize>> {
    let selection_start = selection.column.min(selection.end_column);
    let selection_end = selection.column.max(selection.end_column) + 1;
    let view = alignment.view();

    let mut rel_start: Option<usize> = None;
    let mut rel_end: Option<usize> = None;
    let mut previous_codon_start = None;

    for rel in visible_col_range.clone() {
        let Some(abs) = view.absolute_column_id(rel) else {
            continue;
        };
        let Some(codon_span) = overlay.codon_span(abs) else {
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
        let Some(start) = view.relative_column_id(clipped_start) else {
            continue;
        };
        let Some(end) = view
            .relative_column_id(clipped_end - 1)
            .map(|relative_col| relative_col + 1)
        else {
            continue;
        };
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
        let result = selection_point_crosshair(&model, &viewport, area, 0, 0);
        assert_eq!(result, Some((0, 0)));

        let result = selection_point_crosshair(&model, &viewport, area, 3, 2);
        assert_eq!(result, Some((2, 3)));
    }

    #[test]
    fn crosshair_handles_pinned_band() {
        let mut model = alignment_model(&["s1", "s2", "s3", "s4"]);
        model.pin(0).expect("should pin");

        let mut viewport = Viewport::default();
        viewport.update_dimensions(4, 2, 2);
        viewport.set_bounds(3, 4, 2);

        let area = Rect::new(0, 0, 4, 4);
        let result = selection_point_crosshair(&model, &viewport, area, 0, 0);
        assert_eq!(result, Some((0, 0)));

        let result = selection_point_crosshair(&model, &viewport, area, 0, 1);
        assert!(result.is_none());

        let result = selection_point_crosshair(&model, &viewport, area, 0, 2);
        assert_eq!(result, Some((1, 0)));
    }
}
