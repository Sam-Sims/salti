use std::ops::Range;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::config::theme::{Theme, ThemeStyles};
use crate::core::CoreState;
use crate::core::command::CoreAction;

/// maximum height of the minimap in rows
const MINIMAP_HEIGHT_ROWS: u16 = 7;

/// number of sampled columns per minimap cell when collapsing
const MINIMAP_COLUMN_SAMPLES_PER_CELL: usize = 8;
/// number of sampled sequences per minimap cell when estimating colour.
const MINIMAP_ROW_SAMPLES_PER_CELL: usize = 10;

#[derive(Debug, Clone, Copy)]
pub struct MinimapLayout {
    pub area: Rect,
    pub track_area: Rect,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MinimapState {
    anchor_columns: Option<usize>,
}

impl MinimapState {
    fn position_from_mouse(mouse_x: u16, track_area: Rect, total_columns: usize) -> usize {
        let offset = usize::from(mouse_x - track_area.x);
        let width = usize::from(track_area.width);
        let column = offset * total_columns / width;
        column.min(total_columns - 1)
    }

    fn pan_action(
        mouse_x: u16,
        track_area: Rect,
        total_columns: usize,
        drag_anchor: usize,
    ) -> CoreAction {
        let column = Self::position_from_mouse(mouse_x, track_area, total_columns);
        CoreAction::JumpToPosition(column.saturating_sub(drag_anchor))
    }
    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        overlay_area: Rect,
        viewport_column_range: &Range<usize>,
        total_columns: usize,
    ) -> Option<CoreAction> {
        if total_columns == 0 {
            return None;
        }

        let viewport_cols = viewport_column_range.end - viewport_column_range.start;
        let track_area = layout(overlay_area).track_area;
        let in_track = track_area.contains((mouse.column, mouse.row).into());

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) if in_track => {
                let column = Self::position_from_mouse(mouse.column, track_area, total_columns);
                let drag_anchor = if viewport_column_range.contains(&column) {
                    column - viewport_column_range.start
                } else {
                    viewport_cols / 2
                };

                self.anchor_columns = Some(drag_anchor);
                Some(Self::pan_action(
                    mouse.column,
                    track_area,
                    total_columns,
                    drag_anchor,
                ))
            }
            MouseEventKind::Drag(MouseButton::Left) if in_track => {
                let drag_anchor = self.anchor_columns?;
                Some(Self::pan_action(
                    mouse.column,
                    track_area,
                    total_columns,
                    drag_anchor,
                ))
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let drag_anchor = self.anchor_columns.take()?;
                if in_track {
                    Some(Self::pan_action(
                        mouse.column,
                        track_area,
                        total_columns,
                        drag_anchor,
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

fn sample_alignments(core: &CoreState, column_start: usize, column_end: usize) -> Option<u8> {
    let column_span = column_end.saturating_sub(column_start);
    let row_ids = core.row_visibility.visible_to_absolute();
    let row_count = row_ids.len();
    if row_count == 0 || column_span == 0 {
        return None;
    }

    let row_samples = row_count.min(MINIMAP_ROW_SAMPLES_PER_CELL);
    let column_samples = column_span.clamp(1, MINIMAP_COLUMN_SAMPLES_PER_CELL);
    let mut counts = [0u16; 256];

    for column_sample in 0..column_samples {
        let column_offset = (column_sample * 2 + 1) * column_span / (column_samples * 2);
        let column = (column_start + column_offset).min(column_end - 1);

        for row_sample in 0..row_samples {
            let row_index = row_sample * row_count / row_samples;
            let sequence_id = row_ids[row_index];
            let sequence = &core.data.sequences[sequence_id].alignment.sequence;
            if let Some(&byte) = sequence.get(column) {
                counts[usize::from(byte)] += 1;
            }
        }
    }

    counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| *count)
        .and_then(|(byte, count)| (*count > 0).then_some(byte as u8))
}

fn calculate_block_colour(
    core: &CoreState,
    theme: &Theme,
    column_start: usize,
    column_end: usize,
) -> Color {
    let sequence_type = core.sequence_type();
    sample_alignments(core, column_start, column_end)
        .and_then(|byte| theme.sequence.colour_for(byte, sequence_type))
        .unwrap_or(theme.panel_bg_dim)
}

fn shade_highlight_box(f: &mut Frame, viewport_box: Rect, theme: &Theme) {
    let buffer = f.buffer_mut();
    for position in viewport_box.positions() {
        if let Some(cell) = buffer.cell_mut(position) {
            cell.set_char('▒');
            cell.set_fg(theme.selection_bg);
        }
    }
}

pub fn highlight_box(track_area: Rect, window: Range<usize>, total_columns: usize) -> Option<Rect> {
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

fn render_minimap_track(f: &mut Frame, area: Rect, core: &CoreState, theme: &Theme) {
    let total_columns = core.data.sequence_length;
    let total_width = usize::from(area.width);
    let buffer = f.buffer_mut();

    // render empty block if alignment is empty
    if total_columns == 0 {
        for position in area.positions() {
            if let Some(cell) = buffer.cell_mut(position) {
                cell.set_char(' ');
                cell.set_bg(theme.panel_bg_dim);
            }
        }
        return;
    }

    for block_index in 0..total_width {
        let block_start = block_index * total_columns / total_width;
        let block_end = ((block_index + 1) * total_columns)
            .div_ceil(total_width)
            .max(block_start + 1)
            .min(total_columns);
        let block_colour = calculate_block_colour(core, theme, block_start, block_end);

        let block_x = area.x + block_index as u16;
        let block_area = Rect::new(block_x, area.y, 1, area.height);
        for position in block_area.positions() {
            if let Some(cell) = buffer.cell_mut(position) {
                cell.set_char(' ');
                cell.set_bg(block_colour);
            }
        }
    }
}

pub fn layout(overlay_area: Rect) -> MinimapLayout {
    let height = overlay_area.height.min(MINIMAP_HEIGHT_ROWS);
    let top = overlay_area.y.saturating_add(overlay_area.height - height);
    let area = Rect::new(overlay_area.x, top, overlay_area.width, height);
    let track_area = Block::bordered().inner(area);
    MinimapLayout { area, track_area }
}

pub fn render(
    f: &mut Frame,
    overlay_area: Rect,
    input_area: Rect,
    core: &CoreState,
    theme: &Theme,
    styles: &ThemeStyles,
) {
    let minimap_layout = layout(overlay_area);
    Clear.render(minimap_layout.area, f.buffer_mut());

    let block = Block::bordered()
        .border_style(styles.border)
        .style(styles.panel_block);
    f.render_widget(block, minimap_layout.area);

    render_minimap_track(f, minimap_layout.track_area, core, theme);

    if let Some(viewport_box) = highlight_box(
        minimap_layout.track_area,
        core.viewport.window().col_range,
        core.data.sequence_length,
    ) {
        shade_highlight_box(f, viewport_box, theme);
    }

    f.render_widget(
        Paragraph::new(Line::from(Span::styled("Drag to pan", styles.text_dim)))
            .style(styles.base_block),
        input_area,
    );
}
