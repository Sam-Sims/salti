#![allow(clippy::cast_possible_truncation)]

pub mod bars;
pub mod layout;
pub mod mouse;
pub mod notification;
pub mod panes;
pub mod popups;
pub mod selection;

use std::{num::NonZeroUsize, ops::Range, path::Path, time::Duration};

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, HorizontalAlignment, Rect},
    style::{Color, Style},
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
        popups::{Context, Popup, minimap::MinimapView},
        selection::Selection,
    },
};

const ELLIPSIS: &str = "...";

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
    let preview: Option<Theme> = ui
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
                minimap_view: ui.minimap_view,
            },
        );
    }
}

#[derive(Debug, Default)]
pub struct UiState {
    pub popup: Option<Popup>,
    pub notifications: Notifications,
    pub selection: Option<Selection>,
    pub theme: Theme,
    /// the theme picked from the terminal background, or `None` if it couldn't be detected
    pub auto_theme: Option<ThemeName>,
    pub position: Position,
    /// the alignment being loaded in the background, as the user typed it
    pub loading: Option<String>,
    pub gff_tooltip: Option<String>,
    pub minimap_view: MinimapView,
    pub scroll_step: ScrollStep,
}

impl UiState {
    pub fn notify(&mut self, notification: Notification) {
        let input_background = self.theme.base.bg.unwrap_or(Color::Reset);
        self.notifications.notify(notification, input_background);
    }

    pub fn set_theme(&mut self, choice: ThemeChoice) {
        self.theme = Theme::builtin(choice.resolve(self.auto_theme));
        if choice == ThemeChoice::Auto && self.auto_theme.is_none() {
            self.notify(Notification::error(
                "Could not detect the terminal background. Using a dark theme".to_string(),
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

/// how far Shift+arrows scroll, defaults to 10 but user can override with`set-scroll-step`
#[derive(Debug)]
pub struct ScrollStep(pub NonZeroUsize);

impl Default for ScrollStep {
    fn default() -> Self {
        Self(NonZeroUsize::new(10).expect("10 isn't zero"))
    }
}

/// the file name of an input path, or the input as it is when it has none
pub fn input_name(input: &str) -> &str {
    Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(input)
}

/// the alignment columns that one of `cells` screen cells stands for
///
/// a narrow alignment is stretched, so neighbouring cells can share a column
pub fn cell_columns(cell: usize, cells: usize, total_columns: usize) -> Range<usize> {
    debug_assert!(
        cell < cells,
        "cell {cell} is past the last of {cells} cells"
    );
    let start = cell * total_columns / cells;
    start..(start + 1).max((cell + 1) * total_columns / cells)
}

/// how many eighths of a row a bar `rows` tall fills to show `fraction`
#[allow(clippy::cast_sign_loss, clippy::cast_precision_loss)]
pub fn bar_ticks(fraction: f64, rows: u16) -> u64 {
    debug_assert!(
        (0.0..=1.0).contains(&fraction),
        "bar fraction {fraction} is outside 0 to 1"
    );
    (fraction * full_bar_ticks(rows) as f64).round() as u64
}

pub fn full_bar_ticks(rows: u16) -> u64 {
    u64::from(rows) * 8
}

pub fn render_centred_lines(lines: Vec<Line<'_>>, style: Style, area: Rect, buf: &mut Buffer) {
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    buf.set_style(area, style);
    Paragraph::new(lines)
        .alignment(HorizontalAlignment::Center)
        .style(style)
        .render(area.centered_vertically(Constraint::Length(height)), buf);
}

/// cuts `value` down to `width` cells, ending in `...` when there's room for it
///
/// wide characters are never split, so the result can be a cell short
pub fn truncate_label(value: &str, width: usize) -> String {
    use ratatui::buffer::CellWidth;

    if usize::from(value.cell_width()) <= width {
        return value.to_string();
    }

    let ellipsis = if width > ELLIPSIS.len() { ELLIPSIS } else { "" };
    let budget = width - ellipsis.len();
    let mut text = String::new();
    let mut used = 0;
    for character in value.chars() {
        used += usize::from(character.encode_utf8(&mut [0; 4]).cell_width());
        if used > budget {
            break;
        }
        text.push(character);
    }
    text.push_str(ellipsis);
    text
}

struct EmptyState<'frame> {
    loading: Option<&'frame str>,
    theme: &'frame Theme,
}

impl Widget for EmptyState<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use ratatui::style::Styled;

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

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};
    use rstest::rstest;

    use super::*;
    use crate::{
        session::{DiffMode, ViewState},
        test_utils::{buffer_text, selection, session_with_numbered_ids, ui_state},
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
        ui.notify(Notification::info("Loaded alignment".to_string()));

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
