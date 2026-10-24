use std::{array, iter, ops::Range};

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use libmsa::{AlignmentType, ColumnCounts};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Styled},
    symbols,
    text::Line,
    widgets::{Paragraph, Sparkline, SparklineBar, Widget},
};
use strum::VariantArray;

use crate::{
    command::Command,
    config::{
        keybindings,
        residues::{ResidueColour, ResidueStyles},
        theme::Theme,
    },
    session::Session,
    ui::{
        self, mouse,
        popups::{self, Context, PopupContent, PopupFrame},
        selection,
    },
};

const MINIMAP_HEIGHT_ROWS: u16 = 7;
const MINIMAP_WINDOW_ALPHA: f32 = 0.28;
// width in cells of the sliding window a plot cell takes its peak mean from
const WINDOW_CELLS: usize = 4;

const COLOURS: usize = ResidueColour::VARIANTS.len();

#[derive(Debug)]
pub struct Minimap {
    total_columns: usize,
    composition: Option<Vec<[f32; COLOURS]>>,
    entropy: Vec<f64>,
    differences: Option<Vec<f64>>,
    missing: Option<Vec<f64>>,
}

impl Minimap {
    pub fn new(session: &Session) -> Self {
        let (grid, layout) = (session.grid(), session.layout());
        let alignment_type = grid.alignment_type();
        let is_generic = alignment_type == AlignmentType::Generic;
        let reference = session.state().reference();
        let columns = grid.map_column_counts(layout.rows(), layout.columns(), |counts, column| {
            (
                colour_shares(&counts, alignment_type),
                counts.entropy(),
                reference.map_or(0.0, |row| counts.differing_fraction(grid.cell(row, column))),
                counts.missing_fraction(),
            )
        });
        Self {
            total_columns: layout.columns().len(),
            composition: (!is_generic)
                .then(|| columns.iter().map(|&(shares, ..)| shares).collect()),
            entropy: prefix_sums(columns.iter().map(|&(_, entropy, ..)| entropy)),
            differences: reference
                .is_some()
                .then(|| prefix_sums(columns.iter().map(|&(_, _, differing, _)| differing))),
            missing: (!is_generic)
                .then(|| prefix_sums(columns.iter().map(|&(.., missing)| missing))),
        }
    }
}

impl PopupContent for Minimap {
    fn frame(&self) -> PopupFrame {
        PopupFrame::Bordered
    }

    fn title(&self, width: u16, ctx: &Context) -> Line<'static> {
        popups::tab_title(ctx.minimap_view, width, ctx.theme)
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

    fn handle_key(&mut self, key: KeyEvent, _: Rect, ctx: &Context) -> Vec<Command> {
        let view = ctx.minimap_view;
        let command = match key.code {
            KeyCode::Tab => Some(Command::SetMinimapView(popups::next_tab(view))),
            KeyCode::BackTab => Some(Command::SetMinimapView(popups::previous_tab(view))),
            _ => keybindings::lookup(key).and_then(|command| match command {
                Command::OpenMinimap => Some(Command::SetMinimapView(popups::next_tab(view))),
                Command::ScrollUp { .. }
                | Command::ScrollDown { .. }
                | Command::ScrollLeft { .. }
                | Command::ScrollRight { .. } => Some(command),
                _ => None,
            }),
        };
        command.into_iter().collect()
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, inner: Rect, ctx: &Context) -> Vec<Command> {
        mouse::centre_on_pointer(
            mouse,
            inner,
            ctx.screen.window.columns.len(),
            self.total_columns,
        )
        .into_iter()
        .collect()
    }

    fn render(&self, inner: Rect, buf: &mut Buffer, ctx: &Context) {
        if self.total_columns == 0 {
            return;
        }
        match ctx.minimap_view {
            MinimapView::Composition => {
                let Some(composition) = &self.composition else {
                    render_note(
                        buf,
                        inner,
                        "Composition needs a DNA or protein alignment.",
                        ctx.theme,
                    );
                    return;
                };
                render_composition(buf, inner, composition, &ctx.theme.residues);
            }
            MinimapView::Entropy => {
                render_plot(buf, inner, &self.entropy, ctx.theme.minimap_levels);
            }
            MinimapView::Differences => {
                let Some(differences) = &self.differences else {
                    render_note(buf, inner, "No reference selected.", ctx.theme);
                    return;
                };
                render_plot(buf, inner, differences, ctx.theme.minimap_levels);
            }
            MinimapView::Missing => {
                let Some(missing) = &self.missing else {
                    render_note(
                        buf,
                        inner,
                        "Missing data needs a DNA or protein alignment.",
                        ctx.theme,
                    );
                    return;
                };
                render_plot(buf, inner, missing, ctx.theme.minimap_levels);
            }
        }
        if let Some(window_box) =
            highlight_box(inner, ctx.screen.window.columns.clone(), self.total_columns)
        {
            tint_box(buf, window_box, ctx.theme.minimap_window);
            mark_box(buf, window_box, ctx.theme.text);
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, strum::Display, strum::VariantArray)]
pub enum MinimapView {
    #[default]
    Composition,
    Entropy,
    Differences,
    Missing,
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

fn prefix_sums(values: impl Iterator<Item = f64>) -> Vec<f64> {
    iter::once(0.0)
        .chain(values.scan(0.0, |sum, value| {
            *sum += value;
            Some(*sum)
        }))
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn render_composition(
    buf: &mut Buffer,
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
    let ticks = ui::full_bar_ticks(area.height) as usize;
    for (cell, x) in (0..cells).zip(area.x..) {
        let columns = &composition[ui::cell_columns(cell, cells, composition.len())];
        let mut shares = [0.0; COLOURS];
        for column in columns {
            for (share, column_share) in shares.iter_mut().zip(column) {
                *share += column_share / columns.len() as f32;
            }
        }
        draw_stack(buf, x, area, &colours, &stack_ticks(&shares, ticks));
    }
}

#[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn stack_ticks(shares: &[f32; COLOURS], ticks: usize) -> [usize; COLOURS] {
    let exact = shares.map(|share| f64::from(share) * ticks as f64);
    let mut allocated = exact.map(|exact| exact.floor() as usize);
    let coloured = exact.iter().sum::<f64>().round() as usize;
    let leftover = coloured.saturating_sub(allocated.iter().sum());
    let mut by_remainder: [usize; COLOURS] = array::from_fn(|colour| colour);
    by_remainder.sort_by(|&left, &right| exact[right].fract().total_cmp(&exact[left].fract()));
    for colour in by_remainder.into_iter().take(leftover) {
        allocated[colour] += 1;
    }
    allocated
}

fn draw_stack(buf: &mut Buffer, x: u16, area: Rect, colours: &[Color], ticks: &[usize]) {
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
        let cell = &mut buf[(x, y)];
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

#[allow(clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn render_plot(buf: &mut Buffer, area: Rect, prefix_sums: &[f64], levels: [Color; 4]) {
    let peaks = window_peaks(prefix_sums, usize::from(area.width));
    let peak = peaks.iter().copied().fold(0.0, f64::max);
    if peak <= 0.0 {
        return;
    }
    Sparkline::default()
        .data(peaks.iter().map(|&value| {
            let fraction = value / peak;
            let level = levels[((fraction * levels.len() as f64) as usize).min(levels.len() - 1)];
            SparklineBar::from(ui::bar_ticks(fraction, area.height)).style(Style::new().fg(level))
        }))
        .max(ui::full_bar_ticks(area.height))
        .render(area, buf);
}

#[allow(clippy::cast_precision_loss)]
fn window_peaks(prefix_sums: &[f64], cells: usize) -> Vec<f64> {
    let total_columns = prefix_sums.len() - 1;
    let window = (WINDOW_CELLS * total_columns / cells.max(1)).clamp(1, total_columns);
    let mean = |column: usize| {
        let start = column
            .saturating_sub(window / 2)
            .min(total_columns - window);
        (prefix_sums[start + window] - prefix_sums[start]) / window as f64
    };
    (0..cells)
        .map(|cell| {
            ui::cell_columns(cell, cells, total_columns)
                .map(mean)
                .fold(0.0, f64::max)
        })
        .collect()
}

fn render_note(buf: &mut Buffer, area: Rect, note: &str, theme: &Theme) {
    Paragraph::new(Line::from(note.set_style(theme.text_dim.italic()))).render(area, buf);
}

fn highlight_box(track_area: Rect, window: Range<usize>, total_columns: usize) -> Option<Rect> {
    let cells = usize::from(track_area.width);
    let shows_window = |cell| {
        let columns = ui::cell_columns(cell, cells, total_columns);
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

fn tint_box(buf: &mut Buffer, area: Rect, tint: Color) {
    for position in area.positions() {
        let cell = &mut buf[position];
        if let Some(fg) = selection::blend(cell.fg, tint, MINIMAP_WINDOW_ALPHA) {
            cell.set_fg(fg);
        }
        if let Some(bg) = selection::blend(cell.bg, tint, MINIMAP_WINDOW_ALPHA) {
            cell.set_bg(bg);
        }
    }
}

fn mark_box(buf: &mut Buffer, area: Rect, style: Style) {
    for x in area.left()..area.right() {
        for y in [area.top().saturating_sub(1), area.bottom()] {
            if let Some(cell) = buf.cell_mut((x, y))
                && cell.symbol() == symbols::line::HORIZONTAL
            {
                cell.set_symbol(symbols::line::THICK_HORIZONTAL)
                    .set_style(style);
            }
        }
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
    fn new_has_no_composition_or_missing_for_generic() {
        let mut session = session(&[b"AC"]);
        session
            .update(Position::default(), |state| {
                state.set_alignment_type(AlignmentType::Generic)
            })
            .unwrap();

        let minimap = Minimap::new(&session);
        assert_eq!((minimap.composition, minimap.missing), (None, None));
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

    #[rstest]
    #[case::flat(&[2.0; 4], 2, &[2.0, 2.0])]
    #[case::spike(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0], 12, &[0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0])]
    #[case::spike_at_start(&[4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 12, &[1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])]
    #[case::spike_at_end(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.0], 12, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0])]
    #[case::more_cells_than_columns(&[3.0, 1.0], 8, &[3.0, 3.0, 3.0, 3.0, 1.0, 1.0, 1.0, 1.0])]
    #[case::window_wider_than_alignment(&[4.0, 0.0, 0.0, 0.0], 2, &[1.0, 1.0])]
    fn window_peaks_works(#[case] values: &[f64], #[case] cells: usize, #[case] expected: &[f64]) {
        assert_eq!(
            window_peaks(&prefix_sums(values.iter().copied()), cells),
            expected
        );
    }
}
