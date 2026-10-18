use std::ops::Range;

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::{
    command::Command,
    config::{keybindings, theme::Theme},
    session::Session,
    ui::{
        cell_columns,
        mouse::centre_on_pointer,
        popups::{Context, PopupContent, PopupFrame},
    },
};

/// maximum height of the minimap in rows
const MINIMAP_HEIGHT_ROWS: u16 = 7;

/// number of sampled columns per minimap cell when collapsing
const MINIMAP_COLUMN_SAMPLES_PER_CELL: usize = 8;
const MINIMAP_ROW_SAMPLES: usize = 128;

#[derive(Debug)]
pub(crate) struct Minimap;

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
        render_minimap_track(buf, inner, session, ctx.theme);
        if let Some(window_box) =
            highlight_box(inner, ctx.screen.window.columns.clone(), total_columns)
        {
            shade_highlight_box(buf, window_box, ctx.theme.minimap_window);
        }
    }
}

fn sample(index: usize, samples: usize, len: usize) -> usize {
    (2 * index + 1) * len / (2 * samples)
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

fn render_minimap_track(buffer: &mut Buffer, area: Rect, session: &Session, theme: &Theme) {
    let layout = session.layout();
    let grid = session.grid();
    let (shown_rows, shown_columns) = (layout.rows(), layout.columns());
    let empty = theme.minimap_empty;

    let row_samples = shown_rows.len().min(MINIMAP_ROW_SAMPLES);
    let rows: Vec<usize> = (0..row_samples)
        .map(|index| shown_rows[sample(index, row_samples, shown_rows.len())])
        .collect();
    let column_samples = usize::from(area.width) * MINIMAP_COLUMN_SAMPLES_PER_CELL;

    for (block_index, block_x) in (area.x..area.right()).enumerate() {
        let first = block_index * MINIMAP_COLUMN_SAMPLES_PER_CELL;
        let columns: Vec<usize> = (first..first + MINIMAP_COLUMN_SAMPLES_PER_CELL)
            .map(|index| shown_columns[sample(index, column_samples, shown_columns.len())])
            .collect();
        let block_colour = grid
            .consensus(&rows, &columns, session.consensus_method)
            .and_then(|byte| theme.residues.style(grid.alignment_type(), byte).bg)
            .unwrap_or(empty);

        buffer.set_style(
            Rect::new(block_x, area.y, 1, area.height),
            Style::new().bg(block_colour),
        );
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

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
}
