use ratatui::{
    Frame,
    layout::Rect,
    style::{Styled, Stylize},
    text::Line,
    widgets::Paragraph,
};

use crate::{
    core::{gff::Gff, model::AlignmentModel, stats_cache::ColumnStatsCache},
    ui::{
        layers::render::render_overlays,
        layout::{AppLayout, FrameLayout},
        panes::{
            alignment::AlignmentPane,
            consensus::{ConsensusAlignmentPane, ConsensusSequenceIdPane},
            gff::{GffInfoPane, GffPane},
            sequence_id::SequenceIdPane,
            status_bars::render_frame,
        },
        selection::render_mouse_selection,
        ui_state::{LoadingState, UiState},
    },
};

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
    gff: Option<&Gff>,
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

    if let Some(gff) = gff {
        f.render_widget(
            GffInfoPane {
                tooltip: ui.gff_tooltip.as_deref(),
                theme: &ui.theme,
            },
            layout.gff_info_pane,
        );
        f.render_widget(
            GffPane {
                gff,
                alignment,
                viewport_col_range: &window.col_range,
                theme: &ui.theme,
            },
            layout.gff_pane,
        );
    }

    f.render_widget(
        SequenceIdPane {
            alignment,
            window: &window,
            header: layout.alignment_header,
            theme: &ui.theme,
        },
        layout.sequence_id_pane,
    );

    f.render_widget(
        AlignmentPane {
            alignment,
            viewport: &ui.viewport,
            metrics: stats_cache,
            gff,
            header: layout.alignment_header,
            theme: &ui.theme,
        },
        layout.alignment_pane,
    );

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
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    use super::*;
    use crate::{
        cli::StartupState,
        core::{
            model::{DiffMode, StatsView},
            stats_cache::StatsJobResult,
        },
        ui::{
            layers::{
                notification::{Notification, NotificationLevel},
                palette::CommandPaletteState,
            },
            layout::AlignmentHeaderLayout,
            ui_state::MouseSelection,
        },
    };

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
        let layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );

        terminal
            .draw(|frame| {
                render(
                    frame,
                    alignment,
                    None,
                    ui,
                    stats_cache,
                    &frame_layout,
                    &layout,
                );
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
        let layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );
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

        insta::assert_snapshot!(
            "render_loaded_basic",
            render_text(Some(&alignment), &ui, &metrics, area)
        );

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
        alignment
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap();
        alignment.diff_mode = DiffMode::Reference;
        let metrics = metrics_with(
            StatsView::Translated(libmsa::ReadingFrame::Frame1),
            b"HHHHHH",
            Some(1.0),
        );
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
        ui.layers.open_palette(CommandPaletteState::empty());

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
        let metrics = metrics_with(
            StatsView::Raw,
            b"CATCATCATCATCATCATCATCATCATCATCATCAT",
            Some(1.0),
        );
        let mut ui = ui_state();
        set_viewport(&mut ui, &alignment, area);
        ui.layers.toggle_minimap();

        insta::assert_snapshot!(
            "render_minimap",
            render_text(Some(&alignment), &ui, &metrics, area)
        );
    }
}
