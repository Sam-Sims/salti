use ratatui::{
    Frame,
    layout::Rect,
    style::{Styled, Stylize},
    text::Line,
};

use crate::{
    core::{gff::Gff, session::Session},
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
        utils::render_centred_lines,
    },
};

fn render_empty_state_with_ui(f: &mut Frame, area: Rect, ui: &UiState) {
    let theme = &ui.theme;
    let lines = match &ui.meta.loading_state {
        LoadingState::Failed(error) => vec![Line::from(
            format!("Failed to load alignment: {error}").set_style(theme.styles.error),
        )],
        LoadingState::Idle => vec![
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
        ],
        LoadingState::Loading | LoadingState::Loaded => return,
    };
    render_centred_lines(lines, theme.styles.base_block, area, f.buffer_mut());
}

pub fn render(
    f: &mut Frame,
    session: Option<&Session>,
    gff: Option<&Gff>,
    ui: &UiState,
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
        session,
        ui,
    );
    let Some(session) = session else {
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
    let columns = session.window_columns(ui.window.columns.clone());

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
                session,
                window: &ui.window,
                theme: &ui.theme,
            },
            layout.gff_pane,
        );
    }

    f.render_widget(
        SequenceIdPane {
            session,
            window: &ui.window,
            header: layout.alignment_header,
            theme: &ui.theme,
        },
        layout.sequence_id_pane,
    );
    f.render_widget(
        AlignmentPane {
            session,
            window: &ui.window,
            columns: &columns,
            gff,
            header: layout.alignment_header,
            theme: &ui.theme,
        },
        layout.alignment_pane,
    );
    f.render_widget(
        ConsensusSequenceIdPane {
            session,
            theme: &ui.theme,
        },
        layout.consensus_sequence_id_pane,
    );
    f.render_widget(
        ConsensusAlignmentPane {
            session,
            columns: &columns,
            theme: &ui.theme,
        },
        layout.consensus_alignment_pane,
    );
    render_mouse_selection(f.buffer_mut(), layout, session, ui);

    render_overlays(
        f,
        frame_layout.overlay_area,
        frame_layout.input_area,
        Some(session),
        ui,
    );
}

#[cfg(test)]
mod tests {
    use std::ops;

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        core::session::{DiffMode, Position, ViewState},
        test_utils::{buffer_text, full_window, session_with_ids, ui_state},
        ui::{
            layers::{
                notification::{Notification, NotificationLevel},
                palette::CommandPaletteState,
            },
            layout::AlignmentHeaderLayout,
            selection::Selection,
        },
    };

    const AREA: Rect = Rect::new(0, 0, 100, 24);

    fn seqs(sequences: &[&[u8]]) -> Session {
        let ids: Vec<String> = (1..=sequences.len()).map(|i| format!("seq{i}")).collect();
        let sequences: Vec<(&str, &[u8])> = ids
            .iter()
            .map(String::as_str)
            .zip(sequences.iter().copied())
            .collect();
        session_with_ids(&sequences)
    }

    fn loaded_ui(session: &Session) -> UiState {
        let mut ui = ui_state();
        ui.meta.loading_state = LoadingState::Loaded;
        ui.window = full_window(session);
        ui
    }

    fn render_text(session: Option<&Session>, ui: &UiState) -> String {
        let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
        let frame_layout = FrameLayout::new(AREA);
        let layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );

        terminal
            .draw(|frame| render(frame, session, None, ui, &frame_layout, &layout))
            .unwrap();

        buffer_text(terminal.backend().buffer(), AREA)
    }

    fn selection(
        rows: ops::RangeInclusive<usize>,
        columns: ops::RangeInclusive<usize>,
    ) -> Selection {
        Selection {
            rows: rows.into(),
            columns: columns.into(),
        }
    }

    #[test]
    fn idle() {
        insta::assert_snapshot!(render_text(None, &ui_state()));
    }

    #[test]
    fn failed() {
        let mut ui = ui_state();
        ui.meta.loading_state = LoadingState::Failed("boom".to_string());

        insta::assert_snapshot!(render_text(None, &ui));
    }

    #[test]
    fn loaded() {
        let session = seqs(&[
            b"CATCATCATCATCATCAT",
            b"CATCATGATCATCATCAT",
            b"CATCATCATCATGATCAT",
            b"CATCATCATCATCATCAT",
        ]);

        insta::assert_snapshot!(render_text(Some(&session), &loaded_ui(&session)));
    }

    #[test]
    fn selection_status() {
        let session = seqs(&[
            b"CATCATCATCATCATCAT",
            b"CATCATGATCATCATCAT",
            b"CATCATCATCATGATCAT",
            b"CATCATCATCATCATCAT",
        ]);
        let mut ui = loaded_ui(&session);
        ui.selection = Some(selection(1..=2, 2..=8));

        insta::assert_snapshot!(render_text(Some(&session), &ui));
    }

    #[test]
    fn translated() {
        let mut session = seqs(&[
            b"CATCATCATCATCATCAT",
            b"CATCATGATCATCATCAT",
            b"CATCATCATCATGATCAT",
        ]);
        session
            .update(Position::default(), |state: &mut ViewState| {
                state.set_reference(Some(0));
                state.toggle_translation_overlay()
            })
            .unwrap();
        session.diff_mode = DiffMode::Reference;

        insta::assert_snapshot!(render_text(Some(&session), &loaded_ui(&session)));
    }

    #[test]
    fn notification() {
        let session = seqs(&[b"CATCATCATCATCATCAT", b"CATCATCATCATCATCAT"]);
        let mut ui = loaded_ui(&session);
        ui.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: "Loaded alignment".to_string(),
        });

        insta::assert_snapshot!(render_text(Some(&session), &ui));
    }

    #[test]
    fn palette() {
        let session = seqs(&[b"CATCATCATCATCATCAT", b"CATCATCATCATCATCAT"]);
        let mut ui = loaded_ui(&session);
        ui.layers.open_palette(CommandPaletteState::empty());

        insta::assert_snapshot!(render_text(Some(&session), &ui));
    }
}
