use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Styled, Stylize},
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    core::{columns::WindowColumns, session::Session},
    ui::{rows::stretch, ui_state::ThemeState, utils::render_pane},
};

const CONSERVATION_SPARK_STRS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub(crate) struct ConsensusAlignmentPane<'a> {
    pub(crate) session: &'a Session,
    pub(crate) columns: &'a WindowColumns<'a>,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for ConsensusAlignmentPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = render_pane(&self.theme.styles, None, area, buf);

        let (columns, theme) = (self.columns, self.theme);
        let reference = self.session.state().reference().map_or_else(
            || Line::from("No reference selected.".fg(theme.theme.text_dim).italic()),
            |row| {
                let bytes: Vec<u8> = columns.grid.cells(row, &columns.columns).collect();
                Line::from(stretch(columns, &bytes, None, &theme.sequence))
            },
        );

        let consensus: Vec<u8> = columns
            .summaries
            .iter()
            .map(|s| s.consensus.unwrap_or(b' '))
            .collect();

        let mut lines = vec![
            reference,
            Line::from(stretch(columns, &consensus, None, &theme.sequence)),
        ];

        if shows_conservation_line(self.session) {
            let sparks: String = columns
                .cells
                .iter()
                .map(|cell| cell.map_or(" ", |cell| spark(&columns.summaries[cell.index])))
                .collect();
            lines.push(Line::from(sparks).set_style(theme.styles.accent_alt));
        }

        Paragraph::new(lines)
            .style(theme.styles.base_block)
            .render(inner_area, buf);
    }
}

pub(crate) struct ConsensusSequenceIdPane<'a> {
    pub(crate) session: &'a Session,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for ConsensusSequenceIdPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = render_pane(&self.theme.styles, None, area, buf);

        // keep Conservation at end of array
        let labels = [
            "Reference Sequence:",
            "Consensus Sequence:",
            "Conservation:",
        ];
        let shown = if shows_conservation_line(self.session) {
            3
        } else {
            2
        };
        let lines: Vec<Line> = labels[..shown]
            .iter()
            .map(|&label| Line::from(label.set_style(self.theme.styles.accent)))
            .collect();

        Paragraph::new(lines)
            .style(self.theme.styles.base_block)
            .render(inner_area, buf);
    }
}

fn shows_conservation_line(session: &Session) -> bool {
    session.grid().alignment_type() != libmsa::AlignmentType::Generic
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn spark(summary: &libmsa::ColumnSummary) -> &'static str {
    summary.conservation.map_or(" ", |value| {
        debug_assert!((0.0..=1.0).contains(&value));
        let max_index = CONSERVATION_SPARK_STRS.len() - 1;
        CONSERVATION_SPARK_STRS[(value * max_index as f64).round() as usize]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::session::{Position, ViewState},
        test_utils::{buffer_text, full_window, session},
        ui::layout::{AlignmentHeaderLayout, AppLayout},
    };

    const CAT: &[u8] = b"CATCATCATCATCATCAT";

    fn render_text(session: &Session, area: Rect) -> String {
        let layout = AppLayout::new(area, 0, AlignmentHeaderLayout::without_features());
        let columns = session.window_columns(full_window(session).columns);
        let theme = ThemeState::default();
        let mut buf = Buffer::empty(area);

        ConsensusSequenceIdPane {
            session,
            theme: &theme,
        }
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
