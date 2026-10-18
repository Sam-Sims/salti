use std::ops::Range;

use crossterm::event::{KeyEvent, MouseEvent};
use libmsa::{AlignmentType, ColumnCounts};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Styled},
    symbols,
    text::Line,
    widgets::{Paragraph, Widget},
};
use strum::VariantArray;

use crate::{
    command::Command,
    config::{
        keybindings,
        residues::{ResidueColour, ResidueStyles},
    },
    session::Session,
    ui::{
        cell_columns,
        mouse::centre_on_pointer,
        popups::{Context, PopupContent, PopupFrame},
    },
};

/// maximum height of the minimap in rows
const MINIMAP_HEIGHT_ROWS: u16 = 7;

const COLOURS: usize = ResidueColour::VARIANTS.len();

#[derive(Debug)]
pub(crate) struct Minimap {
    composition: Option<Vec<[f32; COLOURS]>>,
}

impl Minimap {
    pub(crate) fn new(session: &Session) -> Self {
        let (grid, layout) = (session.grid(), session.layout());
        let alignment_type = grid.alignment_type();
        let composition = (alignment_type != AlignmentType::Generic).then(|| {
            grid.map_column_counts(layout.rows(), layout.columns(), |counts, _| {
                colour_shares(&counts, alignment_type)
            })
        });
        Self { composition }
    }
}

impl PopupContent for Minimap {
    fn frame(&self) -> PopupFrame<'_> {
        PopupFrame::Bordered {
            title: "Minimap (drag to pan)",
        }
    }

    fn area(&self, ctx: &Context) -> Rect {
        let overlay_area = ctx.screen.frame.overlay_area;
        let height = overlay_area.height.min(MINIMAP_HEIGHT_ROWS);
        Rect {
            y: overlay_area.bottom() - height,
            height,
            ..overlay_area
        }
    }

    fn handle_key(&mut self, key: KeyEvent, _: Rect, _: &Context) -> Vec<Command> {
        keybindings::lookup(key)
            .filter(|command| {
                matches!(
                    command,
                    Command::ScrollUp { .. }
                        | Command::ScrollDown { .. }
                        | Command::ScrollLeft { .. }
                        | Command::ScrollRight { .. }
                )
            })
            .into_iter()
            .collect()
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, inner: Rect, ctx: &Context) -> Vec<Command> {
        let session = ctx.session.expect("minimap opens only with an alignment");
        let total_columns = session.layout().columns().len();
        centre_on_pointer(mouse, inner, ctx.screen.window.columns.len(), total_columns)
            .into_iter()
            .collect()
    }

    fn render(&self, inner: Rect, buf: &mut Buffer, ctx: &Context) {
        let session = ctx.session.expect("minimap opens only with an alignment");
        let total_columns = session.layout().columns().len();
        if total_columns == 0 {
            return;
        }
        let Some(composition) = &self.composition else {
            Paragraph::new(Line::from(
                "Composition needs a DNA or protein alignment."
                    .set_style(ctx.theme.text_dim.italic()),
            ))
            .render(inner, buf);
            return;
        };
        render_composition(buf, inner, composition, &ctx.theme.residues);
        if let Some(window_box) =
            highlight_box(inner, ctx.screen.window.columns.clone(), total_columns)
        {
            shade_highlight_box(buf, window_box, ctx.theme.minimap_window);
        }
    }
}

fn colour_shares(counts: &ColumnCounts, alignment_type: AlignmentType) -> [f32; COLOURS] {
    let mut shares = [0.0; COLOURS];
    for (byte, share) in counts.composition() {
        if let Some(colour) = ResidueColour::of(alignment_type, byte) {
            shares[colour as usize] += share as f32;
        }
    }
    shares
}

fn shade_highlight_box(buffer: &mut Buffer, viewport_box: Rect, style: Style) {
    for position in viewport_box.positions() {
        if let Some(cell) = buffer.cell_mut(position) {
            cell.set_char('▒');
            cell.set_style(style);
        }
    }
}

fn highlight_box(track_area: Rect, window: Range<usize>, total_columns: usize) -> Option<Rect> {
    let cells = usize::from(track_area.width);
    let shows_window = |cell| {
        let columns = cell_columns(cell, cells, total_columns);
        columns.start.max(window.start) < columns.end.min(window.end)
    };
    let first = (0..cells).position(shows_window)?;
    let last = (0..cells).rposition(shows_window)?;
    Some(Rect {
        x: track_area.x + first as u16,
        width: (last + 1 - first) as u16,
        ..track_area
    })
}

#[allow(clippy::cast_precision_loss)]
fn render_composition(
    buffer: &mut Buffer,
    area: Rect,
    composition: &[[f32; COLOURS]],
    residues: &ResidueStyles,
) {
    let colours: Vec<Color> = ResidueColour::VARIANTS
        .iter()
        .map(|&colour| {
            let style = residues.get(colour);
            style.bg.or(style.fg).unwrap_or_default()
        })
        .collect();
    let cells = usize::from(area.width);
    let ticks = usize::from(area.height) * 8;
    for (cell, x) in (0..cells).zip(area.x..) {
        let columns = &composition[cell_columns(cell, cells, composition.len())];
        let mut shares = [0.0; COLOURS];
        for column in columns {
            for (share, column_share) in shares.iter_mut().zip(column) {
                *share += column_share / columns.len() as f32;
            }
        }
        draw_stack(buffer, x, area, &colours, &stack_ticks(&shares, ticks));
    }
}

#[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn stack_ticks(shares: &[f32; COLOURS], ticks: usize) -> [usize; COLOURS] {
    let exact = shares.map(|share| f64::from(share) * ticks as f64);
    let mut allocated = exact.map(|exact| exact.floor() as usize);
    let coloured = exact.iter().sum::<f64>().round() as usize;
    let leftover = coloured.saturating_sub(allocated.iter().sum());
    let mut by_remainder: Vec<usize> = (0..COLOURS).collect();
    by_remainder.sort_by(|&a, &b| exact[b].fract().total_cmp(&exact[a].fract()));
    for colour in by_remainder.into_iter().take(leftover) {
        allocated[colour] += 1;
    }
    allocated
}

fn draw_stack(buffer: &mut Buffer, x: u16, area: Rect, colours: &[Color], ticks: &[usize]) {
    let colour_at = |tick: usize| {
        let mut end = 0;
        colours.iter().zip(ticks).find_map(|(&colour, &count)| {
            end += count;
            (tick < end).then_some((colour, end))
        })
    };
    for (row, y) in (area.top()..area.bottom()).rev().enumerate() {
        let bottom = row * 8;
        let Some((lower, lower_end)) = colour_at(bottom) else {
            break;
        };
        let eighths = (lower_end - bottom).min(8);
        let cell = &mut buffer[(x, y)];
        cell.set_symbol(eighths_symbol(eighths)).set_fg(lower);
        if eighths < 8
            && let Some((upper, _)) = colour_at(bottom + 7)
        {
            cell.set_bg(upper);
        }
    }
}

fn eighths_symbol(eighths: usize) -> &'static str {
    let levels = symbols::bar::NINE_LEVELS;
    match eighths {
        0 => levels.empty,
        1 => levels.one_eighth,
        2 => levels.one_quarter,
        3 => levels.three_eighths,
        4 => levels.half,
        5 => levels.five_eighths,
        6 => levels.three_quarters,
        7 => levels.seven_eighths,
        _ => levels.full,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{session::Position, test_utils::session};

    fn track(cells: u16) -> Rect {
        Rect::new(10, 2, cells, 5)
    }

    #[rstest]
    #[case::inside_one_cell(10, 100, 32..38, 13, 1)]
    #[case::across_cells(10, 100, 35..55, 13, 3)]
    #[case::whole_alignment(10, 100, 0..100, 10, 10)]
    #[case::column_over_several_cells(5, 3, 1..2, 12, 2)]
    #[case::skips_cell_without_window_columns(2, 3, 1..2, 11, 1)]
    fn highlight_box_works(
        #[case] cells: u16,
        #[case] total_columns: usize,
        #[case] window: Range<usize>,
        #[case] x: u16,
        #[case] width: u16,
    ) {
        assert_eq!(
            highlight_box(track(cells), window, total_columns),
            Some(Rect {
                x,
                width,
                ..track(cells)
            })
        );
    }

    #[rstest]
    #[case::no_cells(0, 5..10)]
    #[case::empty_window(10, 5..5)]
    fn highlight_box_is_none(#[case] cells: u16, #[case] window: Range<usize>) {
        assert_eq!(highlight_box(track(cells), window, 100), None);
    }

    fn by_colour<T: Copy + Default>(values: &[(ResidueColour, T)]) -> [T; COLOURS] {
        let mut by_colour = [T::default(); COLOURS];
        for &(colour, value) in values {
            by_colour[colour as usize] = value;
        }
        by_colour
    }

    #[test]
    fn new_counts_shown_rows_and_columns() {
        let mut session = session(&[b"C-", b"AA"]);
        session
            .update(Position::default(), |state| {
                state.set_reference(Some(1));
                state.set_gap_filter(Some(0.5))
            })
            .unwrap();

        assert_eq!(
            Minimap::new(&session).composition,
            Some(vec![by_colour(&[(ResidueColour::C, 1.0)])])
        );
    }

    #[test]
    fn new_groups_shares_by_colour() {
        let mut session = session(&[b"L", b"V", b"B", b"B"]);
        session
            .update(Position::default(), |state| {
                state.set_alignment_type(AlignmentType::Protein)
            })
            .unwrap();

        assert_eq!(
            Minimap::new(&session).composition,
            Some(vec![by_colour(&[(ResidueColour::Hydrophobic, 0.5)])])
        );
    }

    #[test]
    fn new_has_no_composition_for_generic() {
        let mut session = session(&[b"AC"]);
        session
            .update(Position::default(), |state| {
                state.set_alignment_type(AlignmentType::Generic)
            })
            .unwrap();

        assert_eq!(Minimap::new(&session).composition, None);
    }

    #[rstest]
    #[case::even(&[(ResidueColour::A, 0.5), (ResidueColour::C, 0.5)], 8, &[(ResidueColour::A, 4), (ResidueColour::C, 4)])]
    #[case::largest_remainder(&[(ResidueColour::A, 0.2), (ResidueColour::C, 0.8)], 8, &[(ResidueColour::A, 2), (ResidueColour::C, 6)])]
    #[case::tie_goes_to_earlier(&[(ResidueColour::A, 0.5), (ResidueColour::C, 0.5)], 7, &[(ResidueColour::A, 4), (ResidueColour::C, 3)])]
    #[case::uncoloured_share(&[(ResidueColour::A, 0.5)], 8, &[(ResidueColour::A, 4)])]
    #[case::nothing_counted(&[], 8, &[])]
    fn stack_ticks_works(
        #[case] colour_shares: &[(ResidueColour, f32)],
        #[case] total_ticks: usize,
        #[case] expected: &[(ResidueColour, usize)],
    ) {
        assert_eq!(
            stack_ticks(&by_colour(colour_shares), total_ticks),
            by_colour(expected)
        );
    }
}
