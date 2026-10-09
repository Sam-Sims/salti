use std::ops::Range;

use crossterm::event::MouseEvent;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Widget},
};

use crate::{
    command::Command,
    config::theme::Theme,
    core::session::Session,
    input::movement::HorizontalDrag,
    ui::ui_state::{ThemeState, UiState},
};

/// maximum height of the minimap in rows
const MINIMAP_HEIGHT_ROWS: u16 = 7;

/// number of sampled columns per minimap cell when collapsing
const MINIMAP_COLUMN_SAMPLES_PER_CELL: usize = 8;
const MINIMAP_ROW_SAMPLES: usize = 128;

#[derive(Debug, Clone, Copy)]
struct MinimapLayout {
    area: Rect,
    track_area: Rect,
}

pub struct Minimap<'a> {
    input_area: Rect,
    session: &'a Session,
    ui: &'a UiState,
}

impl<'a> Minimap<'a> {
    pub fn new(input_area: Rect, session: &'a Session, ui: &'a UiState) -> Self {
        Self {
            input_area,
            session,
            ui,
        }
    }
}

impl Widget for Minimap<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let minimap_layout = layout(area);
        let theme = &self.ui.theme.theme;
        let styles = &self.ui.theme.styles;
        let total_columns = self.session.layout().columns().len();

        Clear.render(minimap_layout.area, buffer);
        Block::bordered()
            .border_style(styles.border)
            .style(styles.panel_block)
            .render(minimap_layout.area, buffer);
        if total_columns > 0 {
            render_minimap_track(
                buffer,
                minimap_layout.track_area,
                self.session,
                &self.ui.theme,
            );
            if let Some(window_box) = highlight_box(
                minimap_layout.track_area,
                self.ui.window.columns.clone(),
                total_columns,
            ) {
                shade_highlight_box(buffer, window_box, theme);
            }
        }

        Paragraph::new(Line::from(Span::styled("Drag to pan", styles.text_dim)))
            .style(styles.base_block)
            .render(self.input_area, buffer);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MinimapState {
    pan_drag: HorizontalDrag,
}

impl MinimapState {
    pub fn is_dragging(&self) -> bool {
        self.pan_drag.is_dragging()
    }

    #[allow(clippy::unused_self)]
    pub fn contains_mouse(&self, mouse: MouseEvent, overlay_area: Rect) -> bool {
        let track_area = layout(overlay_area).track_area;
        track_area.contains((mouse.column, mouse.row).into())
    }

    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        overlay_area: Rect,
        viewport_column_range: &Range<usize>,
        total_columns: usize,
    ) -> Option<Command> {
        let track_area = layout(overlay_area).track_area;
        self.pan_drag
            .handle_mouse(mouse, track_area, viewport_column_range, total_columns)
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

        let block_area = Rect::new(block_x, area.y, 1, area.height);
        for position in block_area.positions() {
            if let Some(cell) = buffer.cell_mut(position) {
                cell.set_char(' ');
                cell.set_bg(block_colour);
            }
        }
    }
}

fn layout(overlay_area: Rect) -> MinimapLayout {
    let height = overlay_area.height.min(MINIMAP_HEIGHT_ROWS);
    let top = overlay_area.y.saturating_add(overlay_area.height - height);
    let area = Rect::new(overlay_area.x, top, overlay_area.width, height);
    let track_area = Block::bordered().inner(area);
    MinimapLayout { area, track_area }
}
