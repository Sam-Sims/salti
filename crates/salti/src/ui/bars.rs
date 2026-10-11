use std::fmt::Write as _;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Styled},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::{
    config::theme::ThemeStyles,
    core::session::{Session, ViewMode},
    ui::{input_name, layout::Window, selection::Selection, truncate_label},
};

/// maximum displayed character count for a selected sequence name in the status bar before truncation
const STATUS_BAR_SELECTED_NAME_MAX_CHARS: usize = 25;

pub(crate) struct TopStatusBar<'a> {
    pub(crate) session: Option<&'a Session>,
    pub(crate) window: &'a Window,
    pub(crate) loading: Option<&'a str>,
    pub(crate) styles: &'a ThemeStyles,
}

impl TopStatusBar<'_> {
    fn spans(&self) -> Vec<Span<'static>> {
        let styles = self.styles;
        let session = self.session;
        let file_name = session.map_or("Unknown", |session| input_name(session.source()));
        let loading_status = match (self.loading, session) {
            (Some(input), _) => format!("Loading {}", input_name(input)).set_style(styles.text_dim),
            (None, Some(_)) => Span::styled("Status: Loaded", styles.success),
            (None, None) => Span::styled("Status: Idle", styles.text_dim),
        };

        let layout = session.map(Session::layout);
        let alignment_count = layout.map_or(0, |layout| layout.rows().len());
        let alignment_length = session.map_or(0, |session| session.grid().width());
        let shown = layout
            .map(|layout| &layout.columns()[self.window.columns.clone()])
            .unwrap_or_default();
        let position_range = match (shown.first(), shown.last()) {
            (Some(start), Some(end)) => format!("Positions: {}-{}", start + 1, end + 1),
            _ => "Positions: 0-0".to_string(),
        };

        vec![
            format!("File: {file_name}").set_style(styles.text_dim),
            Span::raw(" | "),
            loading_status,
            Span::raw(" | "),
            format!("{alignment_count} alignments").set_style(styles.text),
            Span::raw(" | "),
            format!("Length: {alignment_length}").set_style(styles.text),
            Span::raw(" | "),
            position_range.set_style(styles.text),
        ]
    }
}

impl Widget for TopStatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(Line::from(self.spans()).right_aligned())
            .style(self.styles.panel_block)
            .render(area, buf);
    }
}

pub(crate) struct BottomStatusBar<'a> {
    pub(crate) session: Option<&'a Session>,
    pub(crate) selection: Option<Selection>,
    pub(crate) styles: &'a ThemeStyles,
}

impl BottomStatusBar<'_> {
    fn spans(&self) -> Vec<Span<'static>> {
        let Some(session) = self.session else {
            return Vec::new();
        };
        let styles = self.styles;
        let (state, layout) = (session.state(), session.layout());
        let filter = state.mode().filter();
        let mut parts = Vec::new();

        if state.row_regex_filter.is_some() || filter.is_active() {
            let mut text = String::from("Filters:");
            if let Some(regex) = &state.row_regex_filter {
                let _ = write!(text, " [rows: {regex}]");
            }
            if let Some(fraction) = filter.max_gap_fraction {
                let _ = write!(text, " [gaps: <= {}%]", format_percent(fraction));
            }
            if let Some(fraction) = filter.min_const_fraction {
                let _ = write!(text, " [constant: >= {}%]", format_percent(fraction));
            }
            let _ = write!(text, " ({} rows)", layout.rows().len());
            if filter.is_active() {
                let _ = write!(text, " ({} cols)", layout.columns().len());
            }
            parts.push(text.set_style(styles.warning));
        }

        if !matches!(state.mode(), ViewMode::Plain { .. }) {
            parts.push(format!("Translation frame: {}", state.frame).set_style(styles.text));
        }

        if let Some(selection) = self.selection {
            let count = selection.rows.last - selection.rows.start + 1;
            let first = layout.columns()[selection.columns.start];
            let last = layout.columns()[selection.columns.last];
            let columns = range_label(first + 1, last + 1);

            let text = if count == 1 {
                let protein = session.protein_columns(first..last + 1);
                let position = if matches!(state.mode(), ViewMode::TranslationOverlay)
                    && !protein.is_empty()
                {
                    range_label(protein.start + 1, protein.end)
                } else {
                    columns
                };
                let name = session
                    .base_alignment()
                    .id(layout.rows()[selection.rows.start]);
                format!(
                    "Selected: {} @ {position}",
                    truncate_label(name, STATUS_BAR_SELECTED_NAME_MAX_CHARS)
                )
            } else {
                format!("{count} sequence(s) selected @ {columns}")
            };
            parts.push(text.set_style(styles.text));
        }

        parts
            .into_iter()
            .flat_map(|part| [Span::raw(" | "), part])
            .skip(1)
            .collect()
    }
}

impl Widget for BottomStatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(Line::from(self.spans()).right_aligned())
            .style(self.styles.panel_block)
            .render(area, buf);
    }
}

pub(crate) struct InputBar<'a> {
    pub(crate) notification: Option<&'a Notification>,
    pub(crate) styles: &'a ThemeStyles,
}

impl Widget for InputBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let line = self
            .notification
            .map_or_else(Line::default, |notification| {
                let (prefix, style) = notification_prefix(notification.level, self.styles);
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(notification.message.as_str(), style),
                ])
            });
        Paragraph::new(line)
            .style(self.styles.base_block)
            .render(area, buf);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    level: NotificationLevel,
    message: String,
}

impl Notification {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Info,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Error,
            message: message.into(),
        }
    }

    pub fn level(&self) -> NotificationLevel {
        self.level
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Error,
    #[allow(dead_code)]
    Warning,
    Info,
}

fn format_percent(fraction: f64) -> String {
    ((fraction * 10_000.0).round() / 100.0).to_string()
}

fn range_label(start: usize, end: usize) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

fn notification_prefix(level: NotificationLevel, styles: &ThemeStyles) -> (&'static str, Style) {
    match level {
        NotificationLevel::Error => ("Error: ", styles.error),
        NotificationLevel::Warning => ("Warning: ", styles.warning),
        NotificationLevel::Info => ("", styles.success),
    }
}

#[cfg(test)]
mod tests {
    use std::ops;

    use super::*;
    use crate::{
        core::session::{Position, ViewState},
        test_utils::{full_window, session_with_numbered_ids},
        ui::state::ThemeState,
    };

    fn status_text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn top(session: &Session, window: &Window, loading: Option<&str>) -> String {
        let theme = ThemeState::default();
        status_text(
            &TopStatusBar {
                session: Some(session),
                window,
                loading,
                styles: &theme.styles,
            }
            .spans(),
        )
    }

    fn bottom(session: &Session, selection: Option<Selection>) -> String {
        let theme = ThemeState::default();
        status_text(
            &BottomStatusBar {
                session: Some(session),
                selection,
                styles: &theme.styles,
            }
            .spans(),
        )
    }

    fn changed(
        sequences: &[&[u8]],
        change: impl FnOnce(&mut ViewState) -> anyhow::Result<()>,
    ) -> Session {
        let mut session = session_with_numbered_ids(sequences);
        session.update(Position::default(), change).unwrap();
        session
    }

    fn bottom_with_selection(
        session: &Session,
        rows: ops::RangeInclusive<usize>,
        columns: ops::RangeInclusive<usize>,
    ) -> String {
        let selection = Selection {
            rows: rows.into(),
            columns: columns.into(),
        };
        bottom(session, Some(selection))
    }

    #[test]
    fn top_loaded() {
        let session = session_with_numbered_ids(&[b"ACGTACGT", b"ACGTAC-T", b"ACGTACGA"]);
        let window = Window {
            columns: 0..5,
            ..full_window(&session)
        };

        insta::assert_snapshot!(top(&session, &window, None));
    }

    #[test]
    fn top_loading_another() {
        let session = session_with_numbered_ids(&[b"ACGTACGT", b"ACGTAC-T", b"ACGTACGA"]);

        insta::assert_snapshot!(top(
            &session,
            &full_window(&session),
            Some("/data/next.fasta")
        ));
    }

    #[test]
    fn bottom_protein_filters() {
        let session = changed(&[b"ATGAAATTT", b"ATG---TTT", b"ATGAAGTTT"], |state| {
            state.row_regex_filter = Some(regex::Regex::new("seq1|seq2").unwrap());
            state.frame = libmsa::ReadingFrame::Frame2;
            state.toggle_protein_view()?;
            state.set_gap_filter(Some(0.5))
        });

        insta::assert_snapshot!(bottom(&session, None));
    }

    #[test]
    fn bottom_constant_filter() {
        let cat: &[u8] = b"CATCATCATCAT";
        let session = changed(&[cat, cat, cat], |state| {
            state.set_constant_filter(Some(1.0))
        });

        insta::assert_snapshot!(bottom(&session, None));
    }

    #[test]
    fn bottom_cell() {
        let session = session_with_numbered_ids(&[b"ACGTACGT", b"ACGTACGT"]);

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=0, 5..=5));
    }

    #[test]
    fn bottom_range() {
        let session = session_with_numbered_ids(&[b"ACGTACGTAC", b"ACGTACGTAC"]);

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=0, 2..=8));
    }

    #[test]
    fn bottom_codon() {
        let session = changed(
            &[b"ATGAAATTT", b"ATGAAATTT"],
            ViewState::toggle_translation_overlay,
        );

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=0, 0..=2));
    }

    #[test]
    fn bottom_translated_range() {
        let session = changed(
            &[b"ATGAAATTT", b"ATGAAATTT"],
            ViewState::toggle_translation_overlay,
        );

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=0, 0..=8));
    }

    #[test]
    fn bottom_rows() {
        let session = session_with_numbered_ids(&[b"ACGTACGT", b"ACGTACGT", b"ACGTACGT"]);

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=2, 1..=4));
    }

    #[test]
    fn bottom_rows_filtered() {
        let session = changed(&[b"ACGTACGT", b"ACGTACGT", b"ACGTACGT"], |state| {
            state.row_regex_filter = Some(regex::Regex::new("seq[13]").unwrap());
            Ok(())
        });

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=1, 1..=4));
    }
}
