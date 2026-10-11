use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Styled, Stylize},
    text::Line,
    widgets::Widget,
};

use crate::{
    core::{gff::Gff, session::Session},
    ui::{
        bars::{BottomStatusBar, InputBar, TopStatusBar},
        input_name,
        layout::ScreenLayout,
        panes::{
            alignment::AlignmentPane,
            consensus::{ConsensusAlignmentPane, ConsensusSequenceIdPane},
            gff::{GffInfoPane, GffPane},
            sequence_id::SequenceIdPane,
        },
        popups::Context,
        render_centred_lines,
        state::{ThemeState, UiState},
    },
};

pub fn render(
    buf: &mut Buffer,
    screen: &ScreenLayout,
    session: Option<&Session>,
    gff: Option<&Gff>,
    ui: &UiState,
) {
    if buf.area.height == 0 {
        return;
    }
    let styles = &ui.theme.styles;
    TopStatusBar {
        session,
        window: &screen.window,
        loading: ui.loading.as_deref(),
        styles,
    }
    .render(screen.frame.top_status_area, buf);
    BottomStatusBar {
        session,
        selection: ui.selection,
        styles,
    }
    .render(screen.frame.bottom_status_area, buf);
    match session {
        Some(session) => render_panes(buf, screen, session, gff, ui),
        None => EmptyState {
            loading: ui.loading.as_deref(),
            theme: &ui.theme,
        }
        .render(screen.frame.content_area, buf),
    }
    InputBar {
        notification: ui.notification(),
        styles,
    }
    .render(screen.frame.input_area, buf);
    if let Some(popup) = &ui.popup {
        popup.render(
            buf,
            &Context {
                session,
                gff,
                theme: &ui.theme,
                screen,
            },
        );
    }
}

struct EmptyState<'a> {
    loading: Option<&'a str>,
    theme: &'a ThemeState,
}

impl Widget for EmptyState<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme;
        let lines = match self.loading {
            Some(input) => vec![Line::from(
                format!("Loading {}", input_name(input)).set_style(theme.styles.text),
            )],
            None => vec![
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
        };
        render_centred_lines(lines, theme.styles.base_block, area, buf);
    }
}

fn render_panes(
    buf: &mut Buffer,
    screen: &ScreenLayout,
    session: &Session,
    gff: Option<&Gff>,
    ui: &UiState,
) {
    let (layout, window) = (&screen.app, &screen.window);
    let columns = session.window_columns(window.columns.clone());

    if let Some(gff) = gff {
        GffInfoPane {
            tooltip: ui.gff_tooltip.as_deref(),
            theme: &ui.theme,
        }
        .render(layout.gff_info_pane, buf);
        GffPane {
            gff,
            session,
            window,
            theme: &ui.theme,
        }
        .render(layout.gff_pane, buf);
    }

    SequenceIdPane {
        session,
        window,
        local_feature_rows: layout.local_feature_rows,
        selection: ui.selection,
        theme: &ui.theme,
    }
    .render(layout.sequence_id_pane, buf);
    AlignmentPane {
        session,
        window,
        columns: &columns,
        gff,
        local_feature_rows: layout.local_feature_rows,
        selection: ui.selection,
        theme: &ui.theme,
    }
    .render(layout.alignment_pane, buf);
    ConsensusSequenceIdPane {
        session,
        theme: &ui.theme,
    }
    .render(layout.consensus_sequence_id_pane, buf);
    ConsensusAlignmentPane {
        session,
        columns: &columns,
        theme: &ui.theme,
    }
    .render(layout.consensus_alignment_pane, buf);
}

#[cfg(test)]
mod tests {
    use std::ops;

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        core::session::{DiffMode, Position, ViewState},
        test_utils::{buffer_text, session_with_numbered_ids, ui_state},
        ui::{bars::Notification, selection::Selection},
    };

    const AREA: Rect = Rect::new(0, 0, 100, 24);

    fn render_text(session: Option<&Session>, ui: &UiState) -> String {
        let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
        let screen = ScreenLayout::new(AREA, session, None, &mut Position::default());

        terminal
            .draw(|frame| render(frame.buffer_mut(), &screen, session, None, ui))
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
    fn loading() {
        let mut ui = ui_state();
        ui.loading = Some("/data/alignment.fasta".to_string());

        insta::assert_snapshot!(render_text(None, &ui));
    }

    #[test]
    fn loaded() {
        let session = session_with_numbered_ids(&[
            b"CATCATCATCATCATCAT",
            b"CATCATGATCATCATCAT",
            b"CATCATCATCATGATCAT",
            b"CATCATCATCATCATCAT",
        ]);

        insta::assert_snapshot!(render_text(Some(&session), &ui_state()));
    }

    #[test]
    fn selection_status() {
        let session = session_with_numbered_ids(&[
            b"CATCATCATCATCATCAT",
            b"CATCATGATCATCATCAT",
            b"CATCATCATCATGATCAT",
            b"CATCATCATCATCATCAT",
        ]);
        let mut ui = ui_state();
        ui.selection = Some(selection(1..=2, 2..=8));

        insta::assert_snapshot!(render_text(Some(&session), &ui));
    }

    #[test]
    fn translated() {
        let mut session = session_with_numbered_ids(&[
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
        session.set_diff_mode(DiffMode::Reference).unwrap();

        insta::assert_snapshot!(render_text(Some(&session), &ui_state()));
    }

    #[test]
    fn notification() {
        let session = session_with_numbered_ids(&[b"CATCATCATCATCATCAT", b"CATCATCATCATCATCAT"]);
        let mut ui = ui_state();
        ui.notify(Notification::info("Loaded alignment"));

        insta::assert_snapshot!(render_text(Some(&session), &ui));
    }
}
