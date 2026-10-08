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

fn spark(summary: &libmsa::ColumnSummary) -> &'static str {
    summary.conservation.map_or(" ", |value| {
        debug_assert!((0.0..=1.0).contains(&value));
        let max_index = CONSERVATION_SPARK_STRS.len() - 1;
        CONSERVATION_SPARK_STRS[(value * max_index as f64).round() as usize]
    })
}

#[cfg(test)]
mod tests {
    use ratatui::{buffer::Buffer, layout::Rect};

    use super::*;
    use crate::{
        core::stats::tests::from_consensus,
        ui::layout::{AlignmentHeaderLayout, AppLayout},
    };

    fn raw(id: &str, sequence: &[u8]) -> libmsa::Sequence {
        libmsa::Sequence {
            id: id.to_string(),
            residues: sequence.to_vec(),
        }
    }

    fn alignment_model(sequences: Vec<libmsa::Sequence>) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(sequences).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    fn render_consensus_pane_text(
        alignment: &AlignmentModel,
        stats: Option<&Stats>,
        area: Rect,
    ) -> String {
        let mut buffer = Buffer::empty(area);
        let layout = AppLayout::new(area, 0, AlignmentHeaderLayout::without_features());
        let window = ViewportWindow {
            row_range: 0..alignment.view().row_count(),
            col_range: 0..alignment.view().column_count(),
            name_range: 0..0,
        };

        let theme = ThemeState::default();
        ConsensusSequenceIdPane {
            alignment,
            theme: &theme,
        }
        .render(layout.consensus_sequence_id_pane, &mut buffer);
        ConsensusAlignmentPane {
            alignment,
            window: &window,
            stats,
            theme: &theme,
        }
        .render(layout.consensus_alignment_pane, &mut buffer);

        buffer_text(&buffer)
    }

    fn buffer_text(buffer: &Buffer) -> String {
        let area = buffer.area;
        let mut lines = Vec::new();

        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                let symbol = buffer[(x, y)].symbol();
                if symbol.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(symbol);
                }
            }
            while line.ends_with(' ') {
                line.pop();
            }
            lines.push(line);
        }

        while matches!(lines.last(), Some(last) if last.is_empty()) {
            lines.pop();
        }

        lines.join("\n")
    }

    #[test]
    fn raw_consensus_pane_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
        ]);
        alignment.set_reference(0).unwrap();
        let stats = from_consensus(0, b"CATCATCATCATCATCAT");

        insta::assert_snapshot!(
            "consensus_pane_raw",
            render_consensus_pane_text(&alignment, Some(&stats), Rect::new(0, 0, 100, 5))
        );
    }

    #[test]
    fn raw_consensus_no_reference_snapshot() {
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
        ]);
        let stats = from_consensus(0, b"CATCATCATCATCATCAT");

        insta::assert_snapshot!(
            "consensus_pane_raw_no_reference",
            render_consensus_pane_text(&alignment, Some(&stats), Rect::new(0, 0, 100, 5))
        );
    }

    #[test]
    fn translated_consensus_pane_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
        ]);
        alignment.set_reference(0).unwrap();
        alignment
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap();
        let stats = from_consensus(0, b"HHHHHH");

        insta::assert_snapshot!(
            "consensus_pane_translated",
            render_consensus_pane_text(&alignment, Some(&stats), Rect::new(0, 0, 100, 5))
        );
    }

    #[test]
    fn translated_consensus_no_reference_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
        ]);
        alignment
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap();
        let stats = from_consensus(0, b"HHHHHH");

        insta::assert_snapshot!(
            "consensus_pane_translated_no_reference",
            render_consensus_pane_text(&alignment, Some(&stats), Rect::new(0, 0, 100, 5))
        );
    }

    #[test]
    fn generic_consensus_hides_conservation_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"ACDEACDEACDE"),
            raw("seq2", b"ACDEACDEACDE"),
        ]);
        alignment.set_reference(0).unwrap();
        let stats = from_consensus(0, b"ACDEACDEACDE");

        insta::assert_snapshot!(
            "consensus_pane_generic_without_conservation",
            render_consensus_pane_text(&alignment, Some(&stats), Rect::new(0, 0, 100, 4))
        );
    }
}
