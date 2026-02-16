use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::macros::vertical;
use ratatui::style::Color::{Reset, Rgb};

use crate::core::CoreState;
use crate::overlay::render_overlays;
use crate::ui::UiState;
use crate::ui::alignment_pane::render_alignment_pane;
use crate::ui::consensus_pane::render_consensus_pane;
use crate::ui::frame::render_frame;
use crate::ui::layout::AppLayout;
use crate::ui::selection::{
    display_index_by_sequence_id, selection_row_bounds, selection_visible_col_range,
    visible_sequence_rows,
};
use crate::ui::sequence_id_pane::render_sequence_id_pane;

fn interpolate(from: u8, to: u8, alpha: f32) -> u8 {
    let from = f32::from(from);
    let to = f32::from(to);
    let blended = (from + (to - from) * alpha).round().clamp(0.0, 255.0);
    if blended <= 0.0 {
        return 0;
    }
    if blended >= 255.0 {
        return u8::MAX;
    }
    blended as u8
}

fn blend_background(
    base: ratatui::style::Color,
    tint: ratatui::style::Color,
    alpha: f32,
) -> ratatui::style::Color {
    match (base, tint) {
        (Rgb(red, green, bloo), Rgb(red_tint, green_tint, bloo_tint)) => Rgb(
            interpolate(red, red_tint, alpha),
            interpolate(green, green_tint, alpha),
            interpolate(bloo, bloo_tint, alpha),
        ),
        (Reset, tint) => tint,
        _ => tint,
    }
}

fn shader(
    f: &mut Frame,
    clip_area: Rect,
    tint_area: Rect,
    tint: ratatui::style::Color,
    alpha: f32,
) {
    if alpha <= 0.0 || clip_area.width == 0 || clip_area.height == 0 {
        return;
    }

    let x_start = tint_area.x.max(clip_area.x);
    let x_end = tint_area
        .x
        .saturating_add(tint_area.width)
        .min(clip_area.x.saturating_add(clip_area.width));
    let y_start = tint_area.y.max(clip_area.y);
    let y_end = tint_area
        .y
        .saturating_add(tint_area.height)
        .min(clip_area.y.saturating_add(clip_area.height));
    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let buffer = f.buffer_mut();
    for y in y_start..y_end {
        for x in x_start..x_end {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_bg(blend_background(cell.bg, tint, alpha));
            }
        }
    }
}

fn render_mouse_selection(f: &mut Frame, layout: &AppLayout, core: &CoreState, ui: &UiState) {
    let Some(selection) = ui.mouse_selection else {
        return;
    };

    let id_inner_area = layout.sequence_id_pane_inner_area();
    let sequence_rows_area = layout.alignment_pane_sequence_rows_area();
    let row_capacity = sequence_rows_area.height as usize;
    let row_ids = visible_sequence_rows(core, row_capacity);
    let display_index_by_sequence_id = display_index_by_sequence_id(core);
    let window = core.viewport.window();
    let (row_min, row_max) = selection_row_bounds(selection, &display_index_by_sequence_id);
    let id_end_x = id_inner_area.x.saturating_add(id_inner_area.width);
    let sequence_end_x = sequence_rows_area
        .x
        .saturating_add(sequence_rows_area.width);

    for (visible_row_index, row_sequence_id) in row_ids.iter().enumerate() {
        let Some(row_sequence_id) = row_sequence_id else {
            continue;
        };
        let display_index = display_index_by_sequence_id[*row_sequence_id];
        if !(row_min..=row_max).contains(&display_index) {
            continue;
        }

        let row_y = sequence_rows_area.y + visible_row_index as u16;
        shader(
            f,
            id_inner_area,
            Rect::new(
                id_inner_area.x,
                row_y,
                id_end_x.saturating_sub(id_inner_area.x),
                1,
            ),
            ui.theme.accent,
            0.3,
        );
        shader(
            f,
            sequence_rows_area,
            Rect::new(
                sequence_rows_area.x,
                row_y,
                sequence_end_x.saturating_sub(sequence_rows_area.x),
                1,
            ),
            ui.theme.surface_bg,
            0.22,
        );
    }

    if let Some(visible_col_range) =
        selection_visible_col_range(selection, window.col_range.clone())
    {
        let start_x =
            sequence_rows_area.x + (visible_col_range.start - window.col_range.start) as u16;
        let end_x_exclusive =
            sequence_rows_area.x + (visible_col_range.end - window.col_range.start) as u16;
        shader(
            f,
            sequence_rows_area,
            Rect::new(
                start_x,
                sequence_rows_area.y,
                end_x_exclusive.saturating_sub(start_x),
                sequence_rows_area.height,
            ),
            ui.theme.panel_bg,
            0.28,
        );
    }
}

pub fn render(f: &mut Frame, core: &CoreState, ui: &UiState) {
    let full_area = f.area();
    if full_area.height == 0 {
        return;
    }

    let [non_input_area, input_area] = full_area.layout(&vertical![*=1, ==1]);
    let [top_status_area, overlay_area] = non_input_area.layout(&vertical![==1, *=1]);
    let [content_area, bottom_status_area] = overlay_area.layout(&vertical![*=1, ==1]);

    let layout = AppLayout::new(content_area);
    // render sequence ID pane (left pane).
    render_sequence_id_pane(f, &layout, core, ui);
    // render alignment pane (top-right pane).
    render_alignment_pane(f, &layout, core, ui);
    // render consensus pane (bottom pane).
    render_consensus_pane(f, &layout, core, ui);
    // mouse selection shader (does nothing if no selection)
    render_mouse_selection(f, &layout, core, ui);
    // status bars on transparent lines
    render_frame(f, top_status_area, bottom_status_area, core, ui);
    // overlays (command palette and input line)
    render_overlays(f, overlay_area, input_area, ui);
}
