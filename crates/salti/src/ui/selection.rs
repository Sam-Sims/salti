use std::range::RangeInclusive;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Color::Rgb, Style},
};

use crate::{
    session::Layout,
    ui::layout::{Window, screen_rows},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub rows: RangeInclusive<usize>,
    pub columns: RangeInclusive<usize>,
}

impl Selection {
    pub(crate) fn shade_rows(
        &self,
        buf: &mut Buffer,
        area: Rect,
        layout: &Layout,
        window: &Window,
        tint: Color,
        alpha: f32,
    ) {
        for (y, position) in (area.top()..area.bottom()).zip(screen_rows(layout, window)) {
            if position.is_some_and(|position| self.rows.contains(&position)) {
                shade(
                    buf,
                    Rect {
                        y,
                        height: 1,
                        ..area
                    },
                    tint,
                    alpha,
                );
            }
        }
    }

    pub(crate) fn shade_columns(
        &self,
        buf: &mut Buffer,
        area: Rect,
        window: &Window,
        tint: Color,
        alpha: f32,
    ) {
        let start = self.columns.start.max(window.columns.start);
        let end = (self.columns.last + 1).min(window.columns.end);
        if start < end {
            let band = Rect {
                x: area.x + (start - window.columns.start) as u16,
                width: (end - start) as u16,
                ..area
            };
            shade(buf, band, tint, alpha);
        }
    }
}

fn shade(buf: &mut Buffer, area: Rect, tint: Color, alpha: f32) {
    for position in area.positions() {
        if let Some(cell) = buf.cell_mut(position) {
            shade_selected_cell(cell, tint, alpha);
        }
    }
}

fn shade_selected_cell(cell: &mut ratatui::buffer::Cell, tint: Color, alpha: f32) {
    match blend(cell.bg, tint, alpha) {
        Some(background) => {
            cell.set_bg(background);
        }
        None => {
            cell.set_style(Style::new().reversed());
        }
    }
}

pub(crate) fn blend(base: Color, tint: Color, alpha: f32) -> Option<Color> {
    match (base, tint) {
        (Rgb(red, green, blue), Rgb(red_tint, green_tint, blue_tint)) => Some(Rgb(
            interpolate(red, red_tint, alpha),
            interpolate(green, green_tint, alpha),
            interpolate(blue, blue_tint, alpha),
        )),
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn interpolate(from: u8, to: u8, alpha: f32) -> u8 {
    let from = f32::from(from);
    let to = f32::from(to);
    (from + (to - from) * alpha).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use std::ops;

    use ratatui::style::Modifier;
    use rstest::rstest;

    use super::*;
    use crate::{session::Session, test_utils::pinned_session};

    fn selection(
        rows: ops::RangeInclusive<usize>,
        columns: ops::RangeInclusive<usize>,
    ) -> Selection {
        Selection {
            rows: rows.into(),
            columns: columns.into(),
        }
    }

    fn highlighted(session: &Session, window: &Window, selection: Selection) -> Vec<String> {
        let id_area = Rect::new(0, 0, 1, 4);
        let rows_area = Rect::new(2, 0, 15, 4);
        let mut buf = Buffer::empty(Rect::new(0, 0, 17, 4));

        selection.shade_rows(
            &mut buf,
            id_area,
            session.layout(),
            window,
            Color::Reset,
            0.5,
        );
        selection.shade_rows(
            &mut buf,
            rows_area,
            session.layout(),
            window,
            Color::Reset,
            0.5,
        );
        selection.shade_columns(&mut buf, rows_area, window, Color::Reset, 0.5);

        let mark = |x, y| {
            if buf[(x, y)].modifier.contains(Modifier::REVERSED) {
                '#'
            } else {
                '.'
            }
        };
        (rows_area.top()..rows_area.bottom())
            .map(|y| {
                let mut line = String::from(mark(id_area.x, y));
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
    fn shade_works(
        #[case] pinned: &[usize],
        #[case] window: Window,
        #[case] selection: Selection,
        #[case] expected: &[&str],
    ) {
        let session = pinned_session(6, pinned);

        assert_eq!(highlighted(&session, &window, selection), expected);
    }
}
