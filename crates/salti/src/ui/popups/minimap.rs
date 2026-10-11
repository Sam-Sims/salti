use std::ops::Range;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::{
    command::Command,
    config::{keybindings, theme::Theme},
    core::session::Session,
    input::centre_on_pointer,
    ui::{
        popups::{Context, PopupContent, PopupFrame},
        state::ThemeState,
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
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                keybindings::lookup(key.code, key.modifiers)
                    .into_iter()
                    .collect()
            }
            _ => Vec::new(),
        }
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
            shade_highlight_box(buf, window_box, &ctx.theme.theme);
        }
    }
}

fn sample(index: usize, samples: usize, len: usize) -> usize {
    (2 * index + 1) * len / (2 * samples)
}

fn shade_highlight_box(buffer: &mut Buffer, viewport_box: Rect, theme: &Theme) {
    for position in viewport_box.positions() {
        if let Some(cell) = buffer.cell_mut(position) {
            cell.set_char('▒');
            cell.set_fg(theme.selection_bg);
        }
    }
}

fn highlight_box(track_area: Rect, window: Range<usize>, total_columns: usize) -> Option<Rect> {
    if track_area.width == 0 {
        return None;
    }

    let width = usize::from(track_area.width);
    let start_offset = (window.start * width / total_columns).min(width - 1);
    let end_offset = (window.end * width)
        .div_ceil(total_columns)
        .max(start_offset + 1)
        .min(width);

    Some(Rect::new(
        track_area.x + start_offset as u16,
        track_area.y,
        (end_offset - start_offset) as u16,
        track_area.height,
    ))
}

fn render_minimap_track(buffer: &mut Buffer, area: Rect, session: &Session, theme: &ThemeState) {
    let layout = session.layout();
    let grid = session.grid();
    let (shown_rows, shown_columns) = (layout.rows(), layout.columns());
    let empty = theme.theme.panel_bg_dim;

    let row_samples = shown_rows.len().min(MINIMAP_ROW_SAMPLES);
    let rows: Vec<usize> = (0..row_samples)
        .map(|index| shown_rows[sample(index, row_samples, shown_rows.len())])
        .collect();
    let column_samples = usize::from(area.width) * MINIMAP_COLUMN_SAMPLES_PER_CELL;
    let byte_styles = theme.sequence.for_type(grid.alignment_type());

    for (block_index, block_x) in (area.x..area.right()).enumerate() {
        let first = block_index * MINIMAP_COLUMN_SAMPLES_PER_CELL;
        let columns: Vec<usize> = (first..first + MINIMAP_COLUMN_SAMPLES_PER_CELL)
            .map(|index| shown_columns[sample(index, column_samples, shown_columns.len())])
            .collect();
        let block_colour = grid
            .consensus(&rows, &columns, session.consensus_method)
            .and_then(|byte| byte_styles[usize::from(byte)].bg)
            .unwrap_or(empty);

        buffer.set_style(
            Rect::new(block_x, area.y, 1, area.height),
            Style::new().bg(block_colour),
        );
    }
}
