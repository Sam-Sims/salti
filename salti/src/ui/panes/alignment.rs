use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    macros::vertical,
    style::{Style, Styled},
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    core::{
        columns::WindowColumns,
        gff::Gff,
        session::{DiffMode, Session},
    },
    ui::{
        layout::{AlignmentHeaderLayout, Window, screen_rows},
        panes::{local_feature_track::LocalFeatureTrack, ruler::Ruler},
        rows::{render_column_scrollbar, stretch},
        ui_state::ThemeState,
        utils::{render_centred_lines, render_pane, separator_line},
    },
};

pub(crate) struct AlignmentPane<'a> {
    pub(crate) session: &'a Session,
    pub(crate) window: &'a Window,
    pub(crate) columns: &'a WindowColumns<'a>,
    pub(crate) gff: Option<&'a Gff>,
    pub(crate) header: AlignmentHeaderLayout,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for AlignmentPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = render_pane(&self.theme.styles, None, area, buf);

        let [local_feature_area, ruler_area, sequence_rows_area] = inner_area.layout(&vertical![
            ==self.header.local_feature_rows,
            ==self.header.ruler_rows,
            *=1
        ]);

        if let Some(gff) = self.gff {
            LocalFeatureTrack {
                gff,
                session: self.session,
                window: self.window,
                theme: self.theme,
            }
            .render(local_feature_area, buf);
        }
        Ruler {
            session: self.session,
            window: self.window,
            theme: self.theme,
        }
        .render(ruler_area, buf);

        let (session, columns, theme) = (self.session, self.columns, self.theme);
        let layout = session.layout();
        render_column_scrollbar(
            "▬",
            Style {
                fg: Some(theme.theme.accent_alt),
                bg: theme.styles.border.fg,
                ..Style::default()
            },
            layout.columns().len(),
            &self.window.columns,
            area.inner(Margin::new(1, 0)),
            buf,
        );

        let empty_message = if layout.columns().is_empty() {
            Some("No columns to display")
        } else if layout.rows().is_empty() {
            Some("No sequences to display")
        } else {
            None
        };
        if let Some(message) = empty_message {
            render_centred_lines(
                vec![Line::from(message.set_style(theme.styles.error))],
                theme.styles.base_block,
                sequence_rows_area,
                buf,
            );
            return;
        }

        let bytes = |row| {
            columns
                .grid
                .cells(row, &columns.columns)
                .collect::<Vec<u8>>()
        };
        let diff = match session.diff_mode {
            DiffMode::Off => None,
            DiffMode::Reference => session.state().reference.map(bytes),
            DiffMode::Consensus => Some(
                columns
                    .summaries
                    .iter()
                    .map(|s| s.consensus.unwrap_or(b' '))
                    .collect(),
            ),
        };
        let lines: Vec<Line> = screen_rows(layout, self.window)
            .map(|pos| match pos {
                Some(pos) => Line::from(stretch(
                    columns,
                    &bytes(layout.rows()[pos]),
                    diff.as_deref(),
                    &theme.sequence,
                )),
                None => separator_line(sequence_rows_area.width, &theme.styles),
            })
            .collect();
        Paragraph::new(lines)
            .style(theme.styles.base_block)
            .render(sequence_rows_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;

    use super::*;
    use crate::{
        core::{
            gff::{Feature, FeatureType, Gff, Strand},
            stats::tests::from_consensus,
        },
        ui::layout::AppLayout,
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

    fn render_alignment_pane_text(
        alignment: &AlignmentModel,
        stats: Option<&Stats>,
        area: Rect,
        row_offset: usize,
        col_offset: usize,
    ) -> String {
        render_alignment_pane_text_with_gff(alignment, stats, None, area, row_offset, col_offset)
    }

    fn render_alignment_pane_text_with_gff(
        alignment: &AlignmentModel,
        stats: Option<&Stats>,
        gff: Option<&Gff>,
        area: Rect,
        row_offset: usize,
        col_offset: usize,
    ) -> String {
        let mut buffer = Buffer::empty(area);
        let header = match gff {
            Some(gff) => {
                let probe_layout =
                    AppLayout::new(area, 0, AlignmentHeaderLayout::without_features());
                let col_range = col_offset
                    ..col_offset
                        .saturating_add(probe_layout.alignment_pane_sequence_rows.width as usize)
                        .min(alignment.view().column_count());
                let local_rows = crate::ui::panes::local_feature_track::local_feature_row_count(
                    gff, alignment, &col_range,
                );
                AlignmentHeaderLayout::with_features(local_rows as u16)
            }
            None => AlignmentHeaderLayout::without_features(),
        };
        let layout = AppLayout::new(area, 0, header);
        let mut viewport = Viewport::default();
        viewport.update_dimensions(
            layout.alignment_pane_sequence_rows.width as usize,
            layout.alignment_pane_sequence_rows.height as usize,
            0,
        );
        viewport.set_bounds(
            alignment.view().row_count(),
            alignment.view().column_count(),
            alignment.base().max_id_len(),
        );
        viewport.offsets.rows = row_offset;
        viewport.offsets.cols = col_offset;

        let theme = ThemeState::default();
        AlignmentPane {
            alignment,
            viewport: &viewport,
            stats,
            gff,
            header: layout.alignment_header,
            theme: &theme,
        }
        .render(layout.alignment_pane, &mut buffer);

        buffer_text(&buffer, layout.alignment_pane)
    }

    fn buffer_text(buffer: &Buffer, area: Rect) -> String {
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
    fn alignment_pane_basic_snapshot() {
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
            raw("seq3", b"CATCATCATCATCATCAT"),
        ]);

        insta::assert_snapshot!(
            "alignment_pane_basic",
            render_alignment_pane_text(&alignment, None, Rect::new(0, 0, 100, 12), 0, 0,)
        );
    }

    #[test]
    fn alignment_pane_reserves_blank_local_feature_row_snapshot() {
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
            raw("seq3", b"CATCATCATCATCATCAT"),
        ]);
        let gff = Gff {
            features: vec![Feature {
                name: "Offscreen".to_string(),
                kind: FeatureType::Gene,
                range: 30..40,
                strand: Strand::Forward,
            }],
        };

        insta::assert_snapshot!(
            "alignment_pane_blank_local_feature_track",
            render_alignment_pane_text_with_gff(
                &alignment,
                None,
                Some(&gff),
                Rect::new(0, 0, 100, 12),
                0,
                0,
            )
        );
    }

    #[test]
    fn alignment_pane_with_local_feature_track_snapshot() {
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
            raw("seq3", b"CATCATCATCATCATCAT"),
        ]);
        let gff = Gff {
            features: vec![Feature {
                name: "Spike".to_string(),
                kind: FeatureType::Gene,
                range: 2..14,
                strand: Strand::Forward,
            }],
        };

        insta::assert_snapshot!(
            "alignment_pane_with_local_feature_track",
            render_alignment_pane_text_with_gff(
                &alignment,
                None,
                Some(&gff),
                Rect::new(0, 0, 100, 12),
                0,
                0,
            )
        );
    }

    #[test]
    fn alignment_pane_pinned_and_fragmented_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CAT---CATCATCATCAT"),
            raw("seq2", b"CAT---CATCATCATCAT"),
            raw("seq3", b"CAT---CATCATCATCAT"),
            raw("seq4", b"CAT---CATCATCATCAT"),
        ]);
        alignment.pin(1).unwrap();
        alignment.pin(3).unwrap();
        alignment.set_gap_filter(Some(0.5)).unwrap();

        insta::assert_snapshot!(
            "alignment_pane_pinned_and_fragmented",
            render_alignment_pane_text(&alignment, None, Rect::new(0, 0, 100, 12), 0, 0,)
        );
    }

    #[test]
    fn alignment_pane_translated_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
            raw("seq3", b"CATCATCATCATCATCAT"),
        ]);
        alignment
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap();

        insta::assert_snapshot!(
            "alignment_pane_translated",
            render_alignment_pane_text(&alignment, None, Rect::new(0, 0, 100, 12), 0, 0,)
        );
    }

    #[test]
    fn alignment_pane_raw_diff_reference_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATGATCATCATCAT"),
            raw("seq3", b"CATCATCATCATGATCAT"),
        ]);
        alignment.set_reference(0).unwrap();
        alignment.diff_mode = DiffMode::Reference;

        insta::assert_snapshot!(
            "alignment_pane_raw_diff_reference",
            render_alignment_pane_text(&alignment, None, Rect::new(0, 0, 100, 12), 0, 0,)
        );
    }

    #[test]
    fn alignment_pane_translated_diff_reference_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATGATCATCATCAT"),
            raw("seq3", b"CATCATCATCATGATCAT"),
        ]);
        alignment.set_reference(0).unwrap();
        alignment
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap();
        alignment.diff_mode = DiffMode::Reference;

        insta::assert_snapshot!(
            "alignment_pane_translated_diff_reference",
            render_alignment_pane_text(&alignment, None, Rect::new(0, 0, 100, 12), 0, 0,)
        );
    }

    #[test]
    fn alignment_pane_raw_diff_consensus_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATGATCATCATCAT"),
            raw("seq3", b"CATCATCATCATGATCAT"),
        ]);
        alignment.diff_mode = DiffMode::Consensus;
        let stats = from_consensus(0, b"CATCATCATCATCATCAT");

        insta::assert_snapshot!(
            "alignment_pane_raw_diff_consensus",
            render_alignment_pane_text(&alignment, Some(&stats), Rect::new(0, 0, 100, 12), 0, 0,)
        );
    }

    #[test]
    fn alignment_pane_scrolled_with_scrollbar_snapshot() {
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCATCATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCATCATCATCATCATCATCAT"),
            raw("seq3", b"CATCATCATCATCATCATCATCATCATCATCATCAT"),
        ]);

        insta::assert_snapshot!(
            "alignment_pane_scrolled_with_scrollbar",
            render_alignment_pane_text(&alignment, None, Rect::new(0, 0, 60, 12), 0, 10,)
        );
    }

    #[test]
    fn alignment_pane_pinned_with_vertical_scroll_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
            raw("seq3", b"CATCATCATCATCATCAT"),
            raw("seq4", b"CATCATCATCATCATCAT"),
            raw("seq5", b"CATCATCATCATCATCAT"),
            raw("seq6", b"CATCATCATCATCATCAT"),
        ]);
        alignment.pin(1).unwrap();
        alignment.pin(4).unwrap();

        insta::assert_snapshot!(
            "alignment_pane_pinned_with_vertical_scroll",
            render_alignment_pane_text(&alignment, None, Rect::new(0, 0, 100, 10), 2, 0,)
        );
    }
}
