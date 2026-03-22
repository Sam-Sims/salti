use crate::{
    core::{model::AlignmentModel, stats_cache::ColumnStatsCache},
    overlay::render::render_overlays,
    ui::{
        alignment_pane::render_alignment_pane,
        consensus_pane::render_consensus_pane,
        frame::render_frame,
        layout::{AppLayout, FrameLayout, RULER_HEIGHT_ROWS, pinned_section_layout},
        selection::{selection_row_bounds, selection_visible_col_range},
        sequence_id_pane::render_sequence_id_pane,
        ui_state::{LoadingState, UiState},
    },
};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color::Rgb;
use ratatui::style::{Styled, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

const SELECTION_ROW_HIGHLIGHT_ALPHA: f32 = 0.3;
const SELECTION_ROW_TINT_ALPHA: f32 = 0.22;
const SELECTION_COL_HIGHLIGHT_ALPHA: f32 = 0.28;

fn interpolate(from: u8, to: u8, alpha: f32) -> u8 {
    let from = f32::from(from);
    let to = f32::from(to);
    (from + (to - from) * alpha).round().clamp(0.0, 255.0) as u8
}

fn blend_background(
    base: ratatui::style::Color,
    tint: ratatui::style::Color,
    alpha: f32,
) -> ratatui::style::Color {
    match (base, tint) {
        (Rgb(red, green, blue), Rgb(red_tint, green_tint, blue_tint)) => Rgb(
            interpolate(red, red_tint, alpha),
            interpolate(green, green_tint, alpha),
            interpolate(blue, blue_tint, alpha),
        ),
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

fn render_mouse_selection(
    f: &mut Frame,
    layout: &AppLayout,
    alignment: &AlignmentModel,
    ui: &UiState,
    viewport: &crate::core::Viewport,
) {
    let Some(selection) = ui.selection else {
        return;
    };

    let window = viewport.window();
    let id_inner_area = Block::bordered().inner(layout.sequence_id_pane);
    let sequence_rows_area = layout.alignment_pane_sequence_rows;
    let id_content_y = id_inner_area.y + RULER_HEIGHT_ROWS;
    let id_end_x = id_inner_area.x.saturating_add(id_inner_area.width);
    let sequence_end_x = sequence_rows_area
        .x
        .saturating_add(sequence_rows_area.width);
    let band_layout = pinned_section_layout(
        alignment.rows().pinned().len(),
        sequence_rows_area.height as usize,
    );
    let (row_min, row_max) = selection_row_bounds(selection);

    for (row_offset, &absolute_row) in alignment
        .rows()
        .pinned()
        .iter()
        .take(band_layout.pinned_rendered)
        .enumerate()
    {
        if !(row_min..=row_max).contains(&absolute_row) {
            continue;
        }

        let row_y = sequence_rows_area.y + row_offset as u16;
        shader(
            f,
            id_inner_area,
            Rect::new(
                id_inner_area.x,
                id_content_y + row_offset as u16,
                id_end_x.saturating_sub(id_inner_area.x),
                1,
            ),
            ui.theme.theme.accent,
            SELECTION_ROW_HIGHLIGHT_ALPHA,
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
            ui.theme.theme.surface_bg,
            SELECTION_ROW_TINT_ALPHA,
        );
    }

    let scroll_start_y = sequence_rows_area.y
        + band_layout.pinned_rendered as u16
        + band_layout.divider_height as u16;
    for (row_offset, relative_row) in window.row_range.clone().enumerate() {
        let Some(absolute_row) = alignment.view().absolute_row_id(relative_row) else {
            continue;
        };
        if !(row_min..=row_max).contains(&absolute_row) {
            continue;
        }

        let row_y = scroll_start_y + row_offset as u16;
        shader(
            f,
            id_inner_area,
            Rect::new(
                id_inner_area.x,
                id_content_y
                    + band_layout.pinned_rendered as u16
                    + band_layout.divider_height as u16
                    + row_offset as u16,
                id_end_x.saturating_sub(id_inner_area.x),
                1,
            ),
            ui.theme.theme.accent,
            SELECTION_ROW_HIGHLIGHT_ALPHA,
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
            ui.theme.theme.surface_bg,
            SELECTION_ROW_TINT_ALPHA,
        );
    }

    if let Some(visible_col_range) =
        selection_visible_col_range(selection, alignment, &window.col_range)
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
            ui.theme.theme.panel_bg,
            SELECTION_COL_HIGHLIGHT_ALPHA,
        );
    }
}

fn render_empty_state_with_ui(f: &mut Frame, area: Rect, ui: &UiState) {
    let theme = &ui.theme;
    match &ui.meta.loading_state {
        LoadingState::Failed(error) => {
            let line = Line::from(
                format!("Failed to load alignment: {error}").set_style(theme.styles.error),
            );
            let centred_area = Rect::new(
                area.x,
                area.y + area.height.saturating_sub(1) / 2,
                area.width,
                area.height.min(1),
            );
            f.render_widget(
                Paragraph::new(line)
                    .alignment(ratatui::layout::HorizontalAlignment::Center)
                    .style(theme.styles.base_block),
                centred_area,
            );
        }
        LoadingState::Idle => {
            let lines = vec![
                Line::from(
                    "salti: A modern MSA browser for the terminal."
                        .fg(theme.theme.text)
                        .bold(),
                ),
                Line::from(
                    "Use the command palette to open an alignment.".set_style(theme.styles.text),
                ),
                Line::from(""),
                Line::from(
                    "Hint: use :load-alignment <alignment.fasta>"
                        .fg(theme.theme.text_dim)
                        .italic(),
                ),
            ];
            let centred_area = Rect::new(
                area.x,
                area.y + area.height.saturating_sub(lines.len() as u16) / 2,
                area.width,
                area.height.min(lines.len() as u16),
            );
            f.render_widget(
                Paragraph::new(lines)
                    .alignment(ratatui::layout::HorizontalAlignment::Center)
                    .style(theme.styles.base_block),
                centred_area,
            );
        }
        LoadingState::Loading | LoadingState::Loaded => {}
    }
}

pub fn render(
    f: &mut Frame,
    alignment: Option<&AlignmentModel>,
    ui: &UiState,
    stats_cache: &ColumnStatsCache,
    frame_layout: &FrameLayout,
    layout: &AppLayout,
) {
    if f.area().height == 0 {
        return;
    }
    render_frame(
        f,
        frame_layout.top_status_area,
        frame_layout.bottom_status_area,
        alignment,
        ui,
    );
    let Some(alignment) = alignment else {
        render_empty_state_with_ui(f, frame_layout.content_area, ui);
        render_overlays(
            f,
            frame_layout.overlay_area,
            frame_layout.input_area,
            None,
            ui,
        );
        return;
    };

    let window = ui.viewport.window();
    render_sequence_id_pane(f, layout, alignment, &window, &ui.theme);

    render_alignment_pane(f, layout, alignment, &ui.viewport, stats_cache, &ui.theme);

    render_consensus_pane(f, layout, alignment, &window, stats_cache, &ui.theme);
    render_mouse_selection(f, layout, alignment, ui, &ui.viewport);

    render_overlays(
        f,
        frame_layout.overlay_area,
        frame_layout.input_area,
        Some(alignment),
        ui,
    );
}
