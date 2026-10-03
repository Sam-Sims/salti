use std::ops::Range;

use crossterm::event::MouseEvent;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Widget},
};
use tracing::warn;

use crate::{
    command::Command,
    config::theme::Theme,
    core::model::AlignmentModel,
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
    alignment: &'a AlignmentModel,
    ui: &'a UiState,
}

impl<'a> Minimap<'a> {
    pub fn new(input_area: Rect, alignment: &'a AlignmentModel, ui: &'a UiState) -> Self {
        Self {
            input_area,
            alignment,
            ui,
        }
    }
}

impl Widget for Minimap<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let minimap_layout = layout(area);
        let theme = &self.ui.theme.theme;
        let styles = &self.ui.theme.styles;
        let total_columns = self.alignment.view().column_count();

        Clear.render(minimap_layout.area, buffer);
        Block::bordered()
            .border_style(styles.border)
            .style(styles.panel_block)
            .render(minimap_layout.area, buffer);
        render_minimap_track(
            buffer,
            minimap_layout.track_area,
            self.alignment,
            &self.ui.theme,
        );

        if let Some(viewport_box) = highlight_box(
            minimap_layout.track_area,
            self.ui.viewport.window().col_range,
            total_columns,
        ) {
            shade_highlight_box(buffer, viewport_box, theme);
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

    pub fn contains_mouse(&self, mouse: MouseEvent, overlay_area: Rect) -> bool {
        let track_area = layout(overlay_area).track_area;
        track_area.contains((mouse.column, mouse.row).into())
    }

    fn position_from_mouse(mouse_x: u16, track_area: Rect, total_columns: usize) -> usize {
        let offset = usize::from(mouse_x.saturating_sub(track_area.x));
        let width = usize::from(track_area.width);
        let column = offset.saturating_mul(total_columns) / width;
        column.min(total_columns.saturating_sub(1))
    }

    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        overlay_area: Rect,
        viewport_column_range: &Range<usize>,
        total_columns: usize,
    ) -> Option<Command> {
        let track_area = layout(overlay_area).track_area;
        self.pan_drag.handle_mouse(
            mouse,
            track_area,
            viewport_column_range,
            total_columns,
            Self::position_from_mouse,
        )
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
    if total_columns == 0 {
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

fn render_minimap_track(
    buffer: &mut Buffer,
    area: Rect,
    alignment: &AlignmentModel,
    theme: &ThemeState,
) {
    let view = alignment.view();
    let row_count = view.row_count();
    let total_columns = view.column_count();
    let total_width = usize::from(area.width);
    let empty = theme.theme.panel_bg_dim;

    let row_samples = row_count.min(MINIMAP_ROW_SAMPLES);
    let rows: Vec<usize> = (0..row_samples)
        .map(|index| sample(index, row_samples, row_count))
        .collect();
    let column_samples = total_width * MINIMAP_COLUMN_SAMPLES_PER_CELL;
    let byte_styles = theme.sequence.for_type(alignment.base().active_type());

    for (block_index, block_x) in (area.x..area.right()).enumerate() {
        let block_colour = if row_count == 0 || total_columns == 0 {
            empty
        } else {
            let first = block_index * MINIMAP_COLUMN_SAMPLES_PER_CELL;
            let columns: Vec<usize> = (first..first + MINIMAP_COLUMN_SAMPLES_PER_CELL)
                .map(|index| sample(index, column_samples, total_columns))
                .collect();
            let consensus = match view.select(&rows, &columns) {
                Ok(block) => block.consensus(alignment.consensus_method),
                Err(error) => {
                    warn!(%error, "Failed to compute minimap colours");
                    None
                }
            };
            consensus
                .and_then(|byte| byte_styles[usize::from(byte)].bg)
                .unwrap_or(empty)
        };

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
