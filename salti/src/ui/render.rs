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
        layout::ScreenLayout,
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
    screen: &ScreenLayout,
    session: Option<&Session>,
    gff: Option<&Gff>,
    ui: &UiState,
) {
    if f.area().height == 0 {
        return;
    }
    render_frame(f, screen, session, ui);
    let Some(session) = session else {
        render_empty_state_with_ui(f, screen.frame.content_area, ui);
        render_overlays(f, screen, None, ui);
        return;
    };
    let (layout, window) = (&screen.app, &screen.window);
    let columns = session.window_columns(window.columns.clone());

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
                window,
                theme: &ui.theme,
            },
            layout.gff_pane,
        );
    }

    f.render_widget(
        SequenceIdPane {
            session,
            window,
            header: layout.alignment_header,
            theme: &ui.theme,
        },
        layout.sequence_id_pane,
    );
    f.render_widget(
        AlignmentPane {
            session,
            window,
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
    render_mouse_selection(f.buffer_mut(), screen, session, ui);

    render_overlays(f, screen, Some(session), ui);
}

#[cfg(test)]
mod tests {
    use std::ops;

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        core::session::{DiffMode, Position, ViewState},
        test_utils::{buffer_text, session_with_ids, ui_state},
        ui::{
            layers::{
                notification::{Notification, NotificationLevel},
                palette::CommandPaletteState,
            },
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

    fn loaded_ui() -> UiState {
        let mut ui = ui_state();
        ui.meta.loading_state = LoadingState::Loaded;
        ui
    }

    fn render_text(session: Option<&Session>, ui: &UiState) -> String {
        let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
        let screen = ScreenLayout::new(AREA, session, None, &mut Position::default());

        terminal
            .draw(|frame| render(frame, &screen, session, None, ui))
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

        insta::assert_snapshot!(render_text(Some(&session), &loaded_ui()));
    }

    #[test]
    fn selection_status() {
        let session = seqs(&[
            b"CATCATCATCATCATCAT",
            b"CATCATGATCATCATCAT",
            b"CATCATCATCATGATCAT",
            b"CATCATCATCATCATCAT",
        ]);
        let mut ui = loaded_ui();
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

        insta::assert_snapshot!(render_text(Some(&session), &loaded_ui()));
    }

    #[test]
    fn notification() {
        let session = seqs(&[b"CATCATCATCATCATCAT", b"CATCATCATCATCATCAT"]);
        let mut ui = loaded_ui();
        ui.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: "Loaded alignment".to_string(),
        });

        insta::assert_snapshot!(render_text(Some(&session), &ui));
    }

    #[test]
    fn palette() {
        let session = seqs(&[b"CATCATCATCATCATCAT", b"CATCATCATCATCATCAT"]);
        let mut ui = loaded_ui();
        ui.layers.open_palette(CommandPaletteState::empty());

        insta::assert_snapshot!(render_text(Some(&session), &ui));
    }
}
