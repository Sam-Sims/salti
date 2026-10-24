use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Styled,
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    config::theme::Theme,
    session::Session,
    ui::{
        layout::{self, Window},
        panes,
        selection::Selection,
    },
};

const SELECTION_ALPHA: f32 = 0.3;

pub struct SequenceIdPane<'frame> {
    pub session: &'frame Session,
    pub window: &'frame Window,
    pub local_feature_rows: u16,
    pub selection: Option<Selection>,
    pub theme: &'frame Theme,
}

impl Widget for SequenceIdPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = panes::render_pane(self.theme, None, area, buf);
        let [_, ruler_area, rows_area] =
            layout::split_alignment_header(inner_area, self.local_feature_rows);

        let (layout, theme) = (self.session.layout(), self.theme);
        if !self.window.pinned.is_empty() {
            Paragraph::new(vec![
                Line::default(),
                Line::from("Pinned sequences:".set_style(theme.text_muted)),
            ])
            .render(ruler_area, buf);
        }

        let names = &self.window.names;
        let lines: Vec<Line> = layout::screen_rows(layout, self.window)
            .map(|row| match row {
                Some(row) => {
                    let base_row = layout.rows()[row];
                    let style = if row < layout.pinned_count() {
                        theme.accent
                    } else {
                        theme.text
                    };

                    let id: String = self
                        .session
                        .base_alignment()
                        .id(base_row)
                        .chars()
                        .skip(names.start)
                        .take(names.len())
                        .collect();
                    Line::from(vec![
                        format!("{} ", base_row + 1).set_style(theme.success),
                        id.set_style(style),
                    ])
                }
                None => panes::separator_line(rows_area.width, theme),
            })
            .collect();
        Paragraph::new(lines)
            .style(theme.base)
            .render(rows_area, buf);

        if let Some(selection) = self.selection {
            selection.shade_rows(
                buf,
                rows_area,
                layout,
                self.window,
                theme.selection_ids,
                SELECTION_ALPHA,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        session::Position,
        test_utils::{buffer_text, full_window, session_with_ids, session_with_numbered_ids},
        ui::layout::AppLayout,
    };

    const CAT: &[u8] = b"CATCATCATCATCATCAT";

    fn render_text(session: &Session, window: &Window, local_feature_rows: u16) -> String {
        let area = Rect::new(0, 0, 150, 12);
        let layout = AppLayout::new(area, 0, local_feature_rows);
        let theme = Theme::default();
        let mut buf = Buffer::empty(area);

        SequenceIdPane {
            session,
            window,
            local_feature_rows,
            selection: None,
            theme: &theme,
        }
        .render(layout.sequence_id_pane, &mut buf);

        buffer_text(&buf, layout.sequence_id_pane)
    }

    #[test]
    fn basic() {
        let session = session_with_numbered_ids(&[CAT; 3]);

        insta::assert_snapshot!(render_text(&session, &full_window(&session), 0));
    }

    #[test]
    fn feature_rows() {
        let session = session_with_numbered_ids(&[CAT; 3]);

        insta::assert_snapshot!(render_text(&session, &full_window(&session), 2));
    }

    #[test]
    fn pinned() {
        let mut session = session_with_numbered_ids(&[CAT; 4]);
        session
            .update(Position::default(), |state| {
                state.pin(1);
                state.pin(3);
                Ok(())
            })
            .unwrap();

        insta::assert_snapshot!(render_text(&session, &full_window(&session), 0));
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

        insta::assert_snapshot!(render_text(&session, &window, 0));
    }
}
