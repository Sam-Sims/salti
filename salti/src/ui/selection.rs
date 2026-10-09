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

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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
    use std::ops;

    use ratatui::style::Modifier;
    use rstest::rstest;

    use super::*;
    use crate::{
        test_utils::{pinned_session, ui_state},
        ui::layout::{AlignmentHeaderLayout, Window},
    };

    fn selection(
        rows: ops::RangeInclusive<usize>,
        columns: ops::RangeInclusive<usize>,
    ) -> Selection {
        Selection {
            rows: rows.into(),
            columns: columns.into(),
        }
    }

    fn highlighted(session: &Session, window: Window, selection: Selection) -> Vec<String> {
        let area = Rect::new(0, 0, 20, 12);
        let layout = AppLayout::new(area, 0, AlignmentHeaderLayout::without_features());
        let mut ui = ui_state();
        ui.window = window;
        ui.selection = Some(selection);
        let mut buf = Buffer::empty(area);

        render_mouse_selection(&mut buf, &layout, session, &ui);

        let id_x = Block::bordered().inner(layout.sequence_id_pane).x;
        let rows_area = layout.alignment_pane_sequence_rows;
        let mark = |x, y| {
            if buf[(x, y)].modifier.contains(Modifier::REVERSED) {
                '#'
            } else {
                '.'
            }
        };
        (rows_area.top()..rows_area.bottom())
            .map(|y| {
                let mut line = String::from(mark(id_x, y));
                line.push('|');
                line.extend((rows_area.left()..rows_area.right()).map(|x| mark(x, y)));
                line
            })
            .collect()
    }

    #[rstest]
    #[case::rows_and_columns(
        &[],
        Window { rows: 0..4, columns: 0..15, ..Window::default() },
        selection(1..=2, 2..=3),
        &[
            ".|..##...........",
            "#|###############",
            "#|###############",
            ".|..##...........",
        ],
    )]
    #[case::scrolled(
        &[],
        Window { rows: 2..6, columns: 3..18, ..Window::default() },
        selection(1..=3, 4..=4),
        &[
            "#|###############",
            "#|###############",
            ".|.#.............",
            ".|.#.............",
        ],
    )]
    #[case::columns_outside_window(
        &[],
        Window { rows: 0..4, columns: 3..18, ..Window::default() },
        selection(0..=0, 0..=1),
        &[
            "#|###############",
            ".|...............",
            ".|...............",
            ".|...............",
        ],
    )]
    #[case::pinned_and_main_skip_divider(
        &[3],
        Window { pinned: 0..1, rows: 0..2, columns: 0..15, ..Window::default() },
        selection(0..=1, 0..=0),
        &[
            "#|###############",
            ".|#..............",
            "#|###############",
            ".|#..............",
        ],
    )]
    fn render_mouse_selection_works(
        #[case] pinned: &[usize],
        #[case] window: Window,
        #[case] selection: Selection,
        #[case] expected: &[&str],
    ) {
        let session = pinned_session(6, pinned);

        assert_eq!(highlighted(&session, window, selection), expected);
    }
}
