#![allow(clippy::cast_possible_truncation)]

pub(crate) mod bars;
pub(crate) mod layout;
pub(crate) mod mouse;
pub(crate) mod notification;
pub(crate) mod panes;
pub(crate) mod popups;
pub(crate) mod selection;

use std::{ops::Range, time::Duration};

use ratatui::{
    buffer::{Buffer, CellWidth},
    layout::{Constraint, HorizontalAlignment, Rect},
    style::{Color, Style, Styled},
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    config::theme::{Theme, ThemeChoice, ThemeName},
    gff::Gff,
    session::{Position, Session},
    ui::{
        bars::{BottomStatusBar, InputBar, TopStatusBar},
        layout::ScreenLayout,
        notification::{Notification, Notifications},
        popups::{Context, Popup},
        selection::Selection,
    },
};

pub fn render(
    buf: &mut Buffer,
    screen: &ScreenLayout,
    session: Option<&Session>,
    gff: Option<&Gff>,
    ui: &mut UiState,
    delta: Duration,
) {
    if buf.area.height == 0 {
        return;
    }
    let preview = ui
        .popup
        .as_ref()
        .and_then(Popup::theme_preview)
        .map(|choice| Theme::builtin(choice.resolve(ui.auto_theme)));
    let theme = preview.as_ref().unwrap_or(&ui.theme);
    TopStatusBar {
        session,
        window: &screen.window,
        loading: ui.loading.as_deref(),
        theme,
    }
    .render(screen.frame.top_status_area, buf);
    BottomStatusBar {
        session,
        selection: ui.selection,
        theme,
    }
    .render(screen.frame.bottom_status_area, buf);
    match session {
        Some(session) => panes::render(buf, screen, session, gff, ui, theme),
        None => EmptyState {
            loading: ui.loading.as_deref(),
            theme,
        }
        .render(screen.frame.content_area, buf),
    }
    InputBar {
        notification: ui.notifications.shown(),
        theme,
    }
    .render(screen.frame.input_area, buf);
    ui.notifications
        .advance(delta, buf, screen.frame.input_area);
    if let Some(popup) = &ui.popup {
        popup.render(
            buf,
            &Context {
                session,
                gff,
                theme,
                screen,
            },
        );
    }
}

#[derive(Debug, Default)]
pub struct UiState {
    pub(crate) popup: Option<Popup>,
    pub(crate) notifications: Notifications,
    pub selection: Option<Selection>,
    pub theme: Theme,
    pub auto_theme: Option<ThemeName>,
    pub position: Position,
    pub loading: Option<String>,
    pub gff_tooltip: Option<String>,
}

impl UiState {
    pub fn notify(&mut self, notification: Notification) {
        let input_bg = self.theme.base.bg.unwrap_or(Color::Reset);
        self.notifications.notify(notification, input_bg);
    }

    pub fn set_theme(&mut self, choice: ThemeChoice) {
        self.theme = Theme::builtin(choice.resolve(self.auto_theme));
        if choice == ThemeChoice::Auto && self.auto_theme.is_none() {
            self.notify(Notification::error(
                "Could not detect the terminal background. Using a dark theme",
            ));
        }
    }

    pub fn clear_transient_state(&mut self) {
        self.selection = None;
        self.popup = None;
        self.notifications = Notifications::default();
        self.gff_tooltip = None;
    }
}

pub(crate) fn input_name(input: &str) -> &str {
    std::path::Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(input)
}

pub(crate) fn cell_columns(cell: usize, cells: usize, total_columns: usize) -> Range<usize> {
    debug_assert!(cell < cells);
    let start = cell * total_columns / cells;
    start..(start + 1).max((cell + 1) * total_columns / cells)
}

struct EmptyState<'a> {
    loading: Option<&'a str>,
    theme: &'a Theme,
}

impl Widget for EmptyState<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme;
        let lines = match self.loading {
            Some(input) => vec![Line::from(
                format!("Loading {}", input_name(input)).set_style(theme.text),
            )],
            None => vec![
                Line::from(
                    "salti: A modern MSA browser for the terminal.".set_style(theme.text.bold()),
                ),
                Line::from("Use the command palette to open an alignment.".set_style(theme.text)),
                Line::from(""),
                Line::from(
                    "Hint: use :load-alignment <alignment.fasta>"
                        .set_style(theme.text_dim.italic()),
                ),
            ],
        };
        render_centred_lines(lines, theme.base, area, buf);
    }
}

fn render_centred_lines(lines: Vec<Line<'_>>, style: Style, area: Rect, buf: &mut Buffer) {
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    buf.set_style(area, style);
    Paragraph::new(lines)
        .alignment(HorizontalAlignment::Center)
        .style(style)
        .render(area.centered_vertically(Constraint::Length(height)), buf);
}

fn truncate_label(value: &str, width: usize) -> String {
    if usize::from(value.cell_width()) <= width {
        return value.to_string();
    }

    let budget = if width <= 3 { width } else { width - 3 };
    let mut used = 0;
    let mut text: String = value
        .chars()
        .take_while(|ch| {
            used += usize::from(ch.encode_utf8(&mut [0; 4]).cell_width());
            used <= budget
        })
        .collect();
    if width > 3 {
        text.push_str("...");
    }
    text
}

#[cfg(test)]
mod tests {
    use std::ops;

    use ratatui::{Terminal, backend::TestBackend};
    use rstest::rstest;

    use super::*;
    use crate::{
        session::{DiffMode, ViewState},
        test_utils::{buffer_text, session_with_numbered_ids, ui_state},
    };

    const AREA: Rect = Rect::new(0, 0, 100, 24);

    fn render_text(session: Option<&Session>, mut ui: UiState) -> String {
        let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
        let screen = ScreenLayout::new(AREA, session, None, &mut Position::default());

        terminal
            .draw(|frame| {
                render(
                    frame.buffer_mut(),
                    &screen,
                    session,
                    None,
                    &mut ui,
                    Duration::ZERO,
                );
            })
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
        insta::assert_snapshot!(render_text(None, ui_state()));
    }

    #[test]
    fn loading() {
        let mut ui = ui_state();
        ui.loading = Some("/data/alignment.fasta".to_string());

        insta::assert_snapshot!(render_text(None, ui));
    }

    #[test]
    fn loaded() {
        let session = session_with_numbered_ids(&[
            b"CATCATCATCATCATCAT",
            b"CATCATGATCATCATCAT",
            b"CATCATCATCATGATCAT",
            b"CATCATCATCATCATCAT",
        ]);

        insta::assert_snapshot!(render_text(Some(&session), ui_state()));
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

        insta::assert_snapshot!(render_text(Some(&session), ui));
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

        insta::assert_snapshot!(render_text(Some(&session), ui_state()));
    }

    #[test]
    fn notification() {
        let session = session_with_numbered_ids(&[b"CATCATCATCATCATCAT", b"CATCATCATCATCATCAT"]);
        let mut ui = ui_state();
        ui.notify(Notification::info("Loaded alignment"));

        insta::assert_snapshot!(render_text(Some(&session), ui));
    }

    #[rstest]
    #[case::fewer_cells(4, 10, vec![0..2, 2..5, 5..7, 7..10])]
    #[case::one_column_per_cell(3, 3, vec![0..1, 1..2, 2..3])]
    #[case::more_cells_repeat_columns(5, 3, vec![0..1, 0..1, 1..2, 1..2, 2..3])]
    fn cell_columns_works(
        #[case] cells: usize,
        #[case] total_columns: usize,
        #[case] expected: Vec<Range<usize>>,
    ) {
        let columns: Vec<_> = (0..cells)
            .map(|cell| cell_columns(cell, cells, total_columns))
            .collect();
        assert_eq!(columns, expected);
    }

    #[rstest]
    #[case::fits("abc", 5, "abc")]
    #[case::exact_fit("abcde", 5, "abcde")]
    #[case::ellipsis("abcdefgh", 6, "abc...")]
    #[case::too_narrow_for_ellipsis("abcdef", 3, "abc")]
    #[case::zero_width("abc", 0, "")]
    #[case::wide_characters("日本語テキスト", 7, "日本...")]
    #[case::wide_character_not_split("日本語", 5, "日...")]
    fn truncate_label_works(#[case] value: &str, #[case] width: usize, #[case] expected: &str) {
        assert_eq!(truncate_label(value, width), expected);
    }
}
