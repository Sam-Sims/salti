use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Styled,
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    core::session::Session,
    ui::{
        layout::{AlignmentHeaderLayout, Window, screen_rows},
        ui_state::ThemeState,
        utils::{render_pane, separator_line},
    },
};

pub(crate) struct SequenceIdPane<'a> {
    pub(crate) session: &'a Session,
    pub(crate) window: &'a Window,
    pub(crate) header: AlignmentHeaderLayout,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for SequenceIdPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = render_pane(&self.theme.styles, None, area, buf);

        let (layout, theme) = (self.session.layout(), self.theme);
        let mut lines = vec![Line::from(" "); usize::from(self.header.height())];
        if self.header.ruler_rows > 1 && !self.window.pinned.is_empty() {
            lines[usize::from(self.header.local_feature_rows) + 1] =
                Line::from("Pinned sequences:".set_style(theme.styles.text_muted));
        }

        let names = &self.window.names;
        lines.extend(screen_rows(layout, self.window).map(|pos| match pos {
            Some(pos) => {
                let row = layout.rows()[pos];
                let style = if pos < layout.pinned() {
                    theme.styles.accent
                } else {
                    theme.styles.text
                };

                let id: String = self
                    .session
                    .base_alignment()
                    .id(row)
                    .chars()
                    .skip(names.start)
                    .take(names.len())
                    .collect();
                Line::from(vec![
                    format!("{} ", row + 1).set_style(theme.styles.success),
                    id.set_style(style),
                ])
            }
            None => separator_line(inner_area.width, &theme.styles),
        }));

        Paragraph::new(lines)
            .style(theme.styles.base_block)
            .render(inner_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::session::Position,
        test_utils::{buffer_text, full_window, session_with_ids},
        ui::layout::AppLayout,
    };

    const CAT: &[u8] = b"CATCATCATCATCATCAT";

    fn render_text(session: &Session, window: &Window, header: AlignmentHeaderLayout) -> String {
        let area = Rect::new(0, 0, 150, 12);
        let layout = AppLayout::new(area, 0, header);
        let theme = ThemeState::default();
        let mut buf = Buffer::empty(area);

        SequenceIdPane {
            session,
            window,
            header,
            theme: &theme,
        }
        .render(layout.sequence_id_pane, &mut buf);

        buffer_text(&buf, layout.sequence_id_pane)
    }

    fn seqs(count: usize) -> Session {
        let ids: Vec<String> = (1..=count).map(|i| format!("seq{i}")).collect();
        let sequences: Vec<(&str, &[u8])> = ids.iter().map(|id| (id.as_str(), CAT)).collect();
        session_with_ids(&sequences)
    }

    #[test]
    fn basic() {
        let session = seqs(3);

        insta::assert_snapshot!(render_text(
            &session,
            &full_window(&session),
            AlignmentHeaderLayout::without_features()
        ));
    }

    #[test]
    fn feature_rows() {
        let session = seqs(3);

        insta::assert_snapshot!(render_text(
            &session,
            &full_window(&session),
            AlignmentHeaderLayout::with_features(2)
        ));
    }

    #[test]
    fn pinned() {
        let mut session = seqs(4);
        session
            .update(Position::default(), |state| {
                state.pin(1);
                state.pin(3);
                Ok(())
            })
            .unwrap();

        insta::assert_snapshot!(render_text(
            &session,
            &full_window(&session),
            AlignmentHeaderLayout::without_features()
        ));
    }

    #[test]
    fn scrolled() {
        let session = session_with_ids(&[
            ("seq1-loooooooooooong-name", CAT),
            ("seq2-loooooooooooong-name", CAT),
            ("seq3-loooooooooooong-name", CAT),
        ]);
        let window = Window {
            names: 5..23,
            ..full_window(&session)
        };

        insta::assert_snapshot!(render_text(
            &session,
            &window,
            AlignmentHeaderLayout::without_features()
        ));
    }
}
