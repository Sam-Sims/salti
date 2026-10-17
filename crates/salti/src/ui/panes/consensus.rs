use ratatui::{
    buffer::Buffer,
    layout::Rect,
    macros::vertical,
    style::Styled,
    text::Line,
    widgets::{Paragraph, Sparkline, Widget},
};

use crate::{
    config::theme::Theme,
    session::{Session, WindowColumns},
    ui::panes::{render_pane, stretch},
};

const FULL_BAR_HEIGHT: u64 = 8;

pub(crate) struct ConsensusAlignmentPane<'a> {
    pub(crate) session: &'a Session,
    pub(crate) columns: &'a WindowColumns<'a>,
    pub(crate) theme: &'a Theme,
}

impl Widget for ConsensusAlignmentPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = render_pane(self.theme, None, area, buf);
        let [sequences_area, conservation_area] = inner_area.layout(&vertical![==2, ==1]);

        let (columns, theme) = (self.columns, self.theme);
        let reference = self.session.state().reference().map_or_else(
            || Line::from("No reference selected.".set_style(theme.text_dim.italic())),
            |row| {
                let bytes: Vec<u8> = columns.grid.cells(row, &columns.columns).collect();
                Line::from(stretch(columns, &bytes, None, theme))
            },
        );
        let lines = vec![
            reference,
            Line::from(stretch(columns, &columns.consensus, None, theme)),
        ];
        Paragraph::new(lines)
            .style(theme.base)
            .render(sequences_area, buf);

        match &columns.conservation {
            Some(conservation) => Sparkline::default()
                .data(
                    columns
                        .cells
                        .iter()
                        .map(|cell| cell.map(|cell| bar_height(conservation[cell.index]))),
                )
                .max(FULL_BAR_HEIGHT)
                .style(theme.accent_alt)
                .render(conservation_area, buf),
            None => Paragraph::new(Line::from(
                "Conservation needs a DNA or protein alignment.".set_style(theme.text_dim.italic()),
            ))
            .style(theme.base)
            .render(conservation_area, buf),
        }
    }
}

pub(crate) struct ConsensusSequenceIdPane<'a> {
    pub(crate) theme: &'a Theme,
}

impl Widget for ConsensusSequenceIdPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = render_pane(self.theme, None, area, buf);

        let lines: Vec<Line> = [
            "Reference Sequence:",
            "Consensus Sequence:",
            "Conservation:",
        ]
        .into_iter()
        .map(|label| Line::from(label.set_style(self.theme.accent)))
        .collect();

        Paragraph::new(lines)
            .style(self.theme.base)
            .render(inner_area, buf);
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn bar_height(value: f64) -> u64 {
    debug_assert!((0.0..=1.0).contains(&value));
    (value * FULL_BAR_HEIGHT as f64).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        session::{Position, ViewState},
        test_utils::{buffer_text, full_window, session},
        ui::layout::AppLayout,
    };

    const CAT: &[u8] = b"CATCATCATCATCATCAT";

    fn render_text(session: &Session, area: Rect) -> String {
        let layout = AppLayout::new(area, 0, 0);
        let columns = session.window_columns(full_window(session).columns);
        let theme = Theme::default();
        let mut buf = Buffer::empty(area);

        ConsensusSequenceIdPane { theme: &theme }
            .render(layout.consensus_sequence_id_pane, &mut buf);
        ConsensusAlignmentPane {
            session,
            columns: &columns,
            theme: &theme,
        }
        .render(layout.consensus_alignment_pane, &mut buf);

        buffer_text(&buf, area)
    }

    fn consensus_text(
        sequences: &[&[u8]],
        change: impl FnOnce(&mut ViewState) -> anyhow::Result<()>,
    ) -> String {
        let mut session = session(sequences);
        session.update(Position::default(), change).unwrap();
        render_text(&session, Rect::new(0, 0, 100, 5))
    }

    #[test]
    fn basic() {
        insta::assert_snapshot!(consensus_text(&[CAT, CAT], |state| {
            state.set_reference(Some(0));
            Ok(())
        }));
    }

    #[test]
    fn no_reference() {
        insta::assert_snapshot!(consensus_text(&[CAT, CAT], |_| Ok(())));
    }

    #[test]
    fn translated() {
        insta::assert_snapshot!(consensus_text(&[CAT, CAT], |state| {
            state.set_reference(Some(0));
            state.toggle_translation_overlay()
        }));
    }

    #[test]
    fn translated_no_reference() {
        insta::assert_snapshot!(consensus_text(
            &[CAT, CAT],
            ViewState::toggle_translation_overlay
        ));
    }

    #[test]
    fn generic() {
        insta::assert_snapshot!(consensus_text(
            &[b"ACDEACDEACDE", b"ACDEACDEACDE"],
            |state| {
                state.set_reference(Some(0));
                state.set_alignment_type(libmsa::AlignmentType::Generic)
            }
        ));
    }
}
