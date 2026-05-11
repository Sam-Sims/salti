use crate::{
    core::{model::AlignmentModel, stats_cache::ColumnStatsCache},
    overlay::render::render_overlays,
    ui::{
        layout::{AppLayout, FrameLayout},
        panes::alignment::render_alignment_pane,
        panes::consensus::{ConsensusAlignmentPane, ConsensusSequenceIdPane},
        panes::gff::{FeatureMap, render as render_gff_pane},
        panes::sequence_id::SequenceIdPane,
        panes::status_bars::render_frame,
        selection::render_mouse_selection,
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

    f.render_widget(
        SequenceIdPane {
            alignment,
            window: &window,
            theme: &ui.theme,
        },
        layout.sequence_id_pane,
    );

    render_alignment_pane(f, layout, alignment, &ui.viewport, stats_cache, &ui.theme);

    f.render_widget(
        ConsensusSequenceIdPane {
            alignment,
            theme: &ui.theme,
        },
        layout.consensus_sequence_id_pane,
    );
    f.render_widget(
        ConsensusAlignmentPane {
            alignment,
            window: &window,
            metrics: stats_cache,
            theme: &ui.theme,
        },
        layout.consensus_alignment_pane,
    );
    render_mouse_selection(f, layout, alignment, ui, &ui.viewport);

    render_overlays(
        f,
        frame_layout.overlay_area,
        frame_layout.input_area,
        Some(alignment),
        ui,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::StartupState;
    use crate::core::model::{DiffMode, StatsView};
    use crate::core::stats_cache::StatsJobResult;
    use crate::overlay::command_palette::CommandPaletteState;
    use crate::ui::notification::{Notification, NotificationLevel};
    use crate::ui::ui_state::MouseSelection;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn alignment_model(sequences: Vec<libmsa::RawSequence>) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(sequences).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    fn metrics_with(
        view: StatsView,
        consensus: &[u8],
        conservation: Option<f32>,
    ) -> ColumnStatsCache {
        let mut cache = ColumnStatsCache::default();
        match view {
            StatsView::Raw => cache.init(consensus.len()),
            StatsView::Translated(frame) => {
                cache.init(consensus.len() * 3);
                let _ =
                    cache.translated_chunks_to_spawn(&(0..consensus.len()), frame, consensus.len());
            }
        }

        let summaries = consensus
            .iter()
            .enumerate()
            .map(|(position, &byte)| libmsa::ColumnSummary {
                position,
                consensus: Some(byte),
                conservation,
            })
            .collect();
        let generation = cache.generation;
        let chunk_idx = 0;
        let stored = cache.store(StatsJobResult {
            generation,
            chunk_idx,
            view,
            summaries: Ok(summaries),
        });
        assert!(stored);
        cache
    }

    fn ui_state() -> UiState {
        let mut ui = UiState::new(StartupState::default());
        ui.meta.loading_state = LoadingState::Loaded;
        ui
    }

    fn render_text(
        alignment: Option<&AlignmentModel>,
        ui: &UiState,
        stats_cache: &ColumnStatsCache,
        area: Rect,
    ) -> String {
        let backend = TestBackend::new(area.width, area.height);
        let mut terminal = Terminal::new(backend).unwrap();
        let frame_layout = FrameLayout::new(area);
        let layout = AppLayout::new(frame_layout.content_area);

        terminal
            .draw(|frame| {
                render(frame, alignment, ui, stats_cache, &frame_layout, &layout);
            })
            .unwrap();

        buffer_text(terminal.backend().buffer(), area)
    }

    fn buffer_text(buffer: &Buffer, area: Rect) -> String {
        let mut lines = Vec::new();

        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                let symbol = buffer[(x, y)].symbol();
                if symbol.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(symbol);
                }
            }
            while line.ends_with(' ') {
                line.pop();
            }
            lines.push(line);
        }

        while matches!(lines.last(), Some(last) if last.is_empty()) {
            lines.pop();
        }

        lines.join("\n")
    }

    fn set_viewport(ui: &mut UiState, alignment: &AlignmentModel, area: Rect) {
        let frame_layout = FrameLayout::new(area);
        let layout = AppLayout::new(frame_layout.content_area);
        ui.viewport.update_dimensions(
            layout.alignment_pane_sequence_rows.width as usize,
            layout.alignment_pane_sequence_rows.height as usize,
            layout.sequence_id_pane.width.saturating_sub(2) as usize,
        );
        ui.viewport.set_bounds(
            alignment.view().row_count(),
            alignment.view().column_count(),
            alignment.base().max_id_len(),
        );
    }

    #[test]
    fn render_empty_state_snapshots() {
        let area = Rect::new(0, 0, 100, 24);

        let idle_ui = UiState::new(StartupState::default());
        insta::assert_snapshot!(
            "render_empty_idle",
            render_text(None, &idle_ui, &ColumnStatsCache::default(), area)
        );

        let mut failed_ui = UiState::new(StartupState::default());
        failed_ui.meta.loading_state = LoadingState::Failed("boom".to_string());
        insta::assert_snapshot!(
            "render_empty_failed",
            render_text(None, &failed_ui, &ColumnStatsCache::default(), area)
        );

        let mut loading_ui = UiState::new(StartupState::default());
        loading_ui.meta.loading_state = LoadingState::Loading;
        insta::assert_snapshot!(
            "render_empty_loading",
            render_text(None, &loading_ui, &ColumnStatsCache::default(), area)
        );
    }

    #[test]
    fn render_loaded_alignment_snapshots() {
        let area = Rect::new(0, 0, 100, 24);
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATGATCATCATCAT"),
            raw("seq3", b"CATCATCATCATGATCAT"),
            raw("seq4", b"CATCATCATCATCATCAT"),
        ]);
        let metrics = metrics_with(StatsView::Raw, b"CATCATCATCATCATCAT", Some(1.0));
        let mut ui = ui_state();
        set_viewport(&mut ui, &alignment, area);

        insta::assert_snapshot!("render_loaded_basic", render_text(Some(&alignment), &ui, &metrics, area));

        let mut selection_ui = ui_state();
        set_viewport(&mut selection_ui, &alignment, area);
        selection_ui.selection = Some(MouseSelection {
            sequence_id: 1,
            column: 2,
            end_sequence_id: 2,
            end_column: 8,
        });
        insta::assert_snapshot!(
            "render_loaded_with_selection_status",
            render_text(Some(&alignment), &selection_ui, &metrics, area)
        );
    }

    #[test]
    fn render_loaded_translation_snapshot() {
        let area = Rect::new(0, 0, 100, 24);
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATGATCATCATCAT"),
            raw("seq3", b"CATCATCATCATGATCAT"),
        ]);
        alignment.set_reference(0).unwrap();
        alignment.set_translation(Some(libmsa::ReadingFrame::Frame1)).unwrap();
        alignment.diff_mode = DiffMode::Reference;
        let metrics = metrics_with(StatsView::Translated(libmsa::ReadingFrame::Frame1), b"HHHHHH", Some(1.0));
        let mut ui = ui_state();
        set_viewport(&mut ui, &alignment, area);

        insta::assert_snapshot!(
            "render_loaded_translation",
            render_text(Some(&alignment), &ui, &metrics, area)
        );
    }

    #[test]
    fn render_notification_snapshot() {
        let area = Rect::new(0, 0, 100, 24);
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
        ]);
        let metrics = metrics_with(StatsView::Raw, b"CATCATCATCATCATCAT", Some(1.0));
        let mut ui = ui_state();
        set_viewport(&mut ui, &alignment, area);
        ui.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: "Loaded alignment".to_string(),
        });

        insta::assert_snapshot!(
            "render_notification",
            render_text(Some(&alignment), &ui, &metrics, area)
        );
    }

    #[test]
    fn render_command_palette_snapshot() {
        let area = Rect::new(0, 0, 100, 24);
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
        ]);
        let metrics = metrics_with(StatsView::Raw, b"CATCATCATCATCATCAT", Some(1.0));
        let mut ui = ui_state();
        set_viewport(&mut ui, &alignment, area);
        ui.overlay.open_palette(CommandPaletteState::empty());

        insta::assert_snapshot!(
            "render_command_palette",
            render_text(Some(&alignment), &ui, &metrics, area)
        );
    }

    #[test]
    fn render_minimap_snapshot() {
        let area = Rect::new(0, 0, 100, 24);
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCATCATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCATCATCATCATCATCATCAT"),
            raw("seq3", b"CATCATCATCATCATCATCATCATCATCATCATCAT"),
        ]);
        let metrics = metrics_with(StatsView::Raw, b"CATCATCATCATCATCATCATCATCATCATCATCAT", Some(1.0));
        let mut ui = ui_state();
        set_viewport(&mut ui, &alignment, area);
        ui.overlay.toggle_minimap();

        insta::assert_snapshot!(
            "render_minimap",
            render_text(Some(&alignment), &ui, &metrics, area)
        );
    }
}
