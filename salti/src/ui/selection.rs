use std::range::RangeInclusive;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Color::Rgb, Style},
    widgets::Block,
};

use crate::{
    core::session::Session,
    ui::{
        layout::{AppLayout, screen_rows},
        ui_state::UiState,
    },
};

const SELECTION_ROW_HIGHLIGHT_ALPHA: f32 = 0.3;
const SELECTION_ROW_TINT_ALPHA: f32 = 0.22;
const SELECTION_COL_HIGHLIGHT_ALPHA: f32 = 0.28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub rows: RangeInclusive<usize>,
    pub columns: RangeInclusive<usize>,
}

pub fn render_mouse_selection(
    buf: &mut Buffer,
    layout: &AppLayout,
    session: &Session,
    ui: &UiState,
) {
    let Some(selection) = ui.selection else {
        return;
    };

    let window = &ui.window;
    let id_area = Block::bordered().inner(layout.sequence_id_pane);
    let rows_area = layout.alignment_pane_sequence_rows;

    for (y, position) in (rows_area.y..).zip(screen_rows(session.layout(), window)) {
        if position.is_some_and(|position| selection.rows.contains(&position)) {
            shader(
                buf,
                Rect {
                    y,
                    height: 1,
                    ..id_area
                },
                ui.theme.theme.accent,
                SELECTION_ROW_HIGHLIGHT_ALPHA,
            );
            shader(
                buf,
                Rect {
                    y,
                    height: 1,
                    ..rows_area
                },
                ui.theme.theme.surface_bg,
                SELECTION_ROW_TINT_ALPHA,
            );
        }
    }

    let start = selection.columns.start.max(window.columns.start);
    let end = (selection.columns.last + 1).min(window.columns.end);
    if start < end {
        let band = Rect {
            x: rows_area.x + (start - window.columns.start) as u16,
            width: (end - start) as u16,
            ..rows_area
        };
        shader(
            buf,
            band,
            ui.theme.theme.panel_bg,
            SELECTION_COL_HIGHLIGHT_ALPHA,
        );
    }
}

fn interpolate(from: u8, to: u8, alpha: f32) -> u8 {
    let from = f32::from(from);
    let to = f32::from(to);
    (from + (to - from) * alpha).round().clamp(0.0, 255.0) as u8
}

fn blend_background(base: Color, tint: Color, alpha: f32) -> Option<Color> {
    match (base, tint) {
        (Rgb(red, green, blue), Rgb(red_tint, green_tint, blue_tint)) => Some(Rgb(
            interpolate(red, red_tint, alpha),
            interpolate(green, green_tint, alpha),
            interpolate(blue, blue_tint, alpha),
        )),
        _ => None,
    }
}

fn shade_selected_cell(cell: &mut ratatui::buffer::Cell, tint: Color, alpha: f32) {
    match blend_background(cell.bg, tint, alpha) {
        Some(background) => {
            cell.set_bg(background);
        }
        None => {
            cell.set_style(Style::new().reversed());
        }
    }
}

fn shader(buf: &mut Buffer, area: Rect, tint: Color, alpha: f32) {
    for position in area.positions() {
        if let Some(cell) = buf.cell_mut(position) {
            shade_selected_cell(cell, tint, alpha);
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::core::{Viewport, model::AlignmentModel};

    fn raw(id: &str, sequence: &[u8]) -> libmsa::Sequence {
        libmsa::Sequence {
            id: id.to_string(),
            residues: sequence.to_vec(),
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
