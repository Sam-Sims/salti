use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Styled, Stylize},
    symbols::merge::MergeStrategy,
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

use crate::{
    core::{
        codon::{TranslatedByteRange, TranslationOverlay, nuc_start},
        model::AlignmentModel,
        stats_cache::ColumnStatsCache,
        viewport::ViewportWindow,
    },
    ui::{
        rows::{
            RowRenderMode, format_row_spans, format_row_view_spans,
            format_translated_byte_range_spans, format_translated_row_spans,
        },
        ui_state::ThemeState,
    },
};

const CONSERVATION_SPARK_STRS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub(crate) struct ConsensusAlignmentPane<'a> {
    pub(crate) alignment: &'a AlignmentModel,
    pub(crate) window: &'a ViewportWindow,
    pub(crate) metrics: &'a ColumnStatsCache,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for ConsensusAlignmentPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_style(self.theme.styles.border)
            .style(self.theme.styles.base_block)
            .merge_borders(MergeStrategy::Exact);
        let inner_area = block.inner(area);
        block.render(area, buf);

        let lines =
            consensus_alignment_lines(self.alignment, self.window, self.metrics, self.theme);
        Paragraph::new(lines)
            .style(self.theme.styles.base_block)
            .render(inner_area, buf);
    }
}

pub(crate) struct ConsensusSequenceIdPane<'a> {
    pub(crate) alignment: &'a AlignmentModel,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for ConsensusSequenceIdPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_style(self.theme.styles.border)
            .style(self.theme.styles.base_block)
            .merge_borders(MergeStrategy::Exact);
        let inner_area = block.inner(area);
        block.render(area, buf);

        let lines = if shows_conservation_line(self.alignment) {
            vec![
                Line::from("Reference Sequence:".set_style(self.theme.styles.accent)),
                Line::from("Consensus Sequence:".set_style(self.theme.styles.accent)),
                Line::from("Conservation:".set_style(self.theme.styles.accent)),
            ]
        } else {
            vec![
                Line::from("Reference Sequence:".set_style(self.theme.styles.accent)),
                Line::from("Consensus Sequence:".set_style(self.theme.styles.accent)),
            ]
        };

        Paragraph::new(lines)
            .style(self.theme.styles.base_block)
            .render(inner_area, buf);
    }
}

fn conservation_to_spark(value: f32) -> &'static str {
    let value = value.clamp(0.0, 1.0);
    let max_idx = CONSERVATION_SPARK_STRS.len() - 1;
    let idx = (value * max_idx as f32).round() as usize;
    CONSERVATION_SPARK_STRS[idx]
}

fn shows_conservation_line(alignment: &AlignmentModel) -> bool {
    alignment.base().active_type() != libmsa::AlignmentType::Generic
}

fn blank_line(width: usize) -> Line<'static> {
    Line::raw(" ".repeat(width))
}

fn translated_reference_line(
    alignment: &AlignmentModel,
    overlay: &TranslationOverlay,
    window: &ViewportWindow,
    theme: &ThemeState,
) -> Line<'static> {
    let Some(translated) = alignment.translated_view() else {
        return Line::from("No reference selected".fg(theme.theme.text_dim).italic());
    };

    alignment.rows().reference().map_or_else(
        || Line::from("No reference selected".fg(theme.theme.text_dim).italic()),
        |absolute_row| {
            let Some(sequence) = translated.project_absolute_row(absolute_row) else {
                return Line::from("No reference selected".fg(theme.theme.text_dim).italic());
            };
            let spans = format_translated_row_spans(
                sequence,
                &window.col_range,
                overlay,
                &theme.theme.sequence,
                None,
            );
            Line::from(spans)
        },
    )
}

fn translated_consensus_line(
    overlay: &TranslationOverlay,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    theme: &ThemeState,
) -> Line<'static> {
    let Some(protein_range) = overlay.visible_protein_range(&window.col_range) else {
        return blank_line(window.col_range.len());
    };

    let consensus_bytes: Option<Vec<u8>> = protein_range
        .clone()
        .map(|protein_col: usize| {
            metrics
                .translated_summary_at(overlay.frame, protein_col)
                .map(|summary| summary.consensus.unwrap_or(b' '))
        })
        .collect();
    let Some(consensus_bytes) = consensus_bytes else {
        return Line::from("Calculating consensus...".fg(theme.theme.text_dim).italic());
    };
    let spans = format_translated_byte_range_spans(
        TranslatedByteRange::new(protein_range.start, &consensus_bytes),
        &window.col_range,
        overlay,
        &theme.theme.sequence,
        None,
    );
    Line::from(spans)
}

fn translated_conservation_line(
    overlay: &TranslationOverlay,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    theme: &ThemeState,
) -> Line<'static> {
    let width = window.col_range.len();
    let mut spans = vec![ratatui::text::Span::styled(" ", theme.styles.accent_alt); width];

    let Some(protein_range) = overlay.visible_protein_range(&window.col_range) else {
        return Line::from(spans);
    };

    for protein_col in protein_range {
        let Some(summary) = metrics.translated_summary_at(overlay.frame, protein_col) else {
            return Line::from(
                "Calculating conservation..."
                    .fg(theme.theme.text_dim)
                    .italic(),
            );
        };
        let spark = summary
            .conservation
            .filter(|value| value.is_finite())
            .map_or(" ", conservation_to_spark);
        let codon_nuc_start = nuc_start(protein_col, overlay.frame);

        for absolute_col in codon_nuc_start..=codon_nuc_start + 2 {
            let Some(window_offset) = absolute_col.checked_sub(window.col_range.start) else {
                continue;
            };
            if window_offset >= width {
                continue;
            }

            spans[window_offset] = ratatui::text::Span::styled(spark, theme.styles.accent_alt);
        }
    }

    Line::from(spans)
}

fn consensus_alignment_lines(
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    theme: &ThemeState,
) -> Vec<Line<'static>> {
    if let Some(overlay) = alignment.translation_overlay() {
        return vec![
            translated_reference_line(alignment, &overlay, window, theme),
            translated_consensus_line(&overlay, window, metrics, theme),
            translated_conservation_line(&overlay, window, metrics, theme),
        ];
    }

    let no_diff_mode = RowRenderMode {
        alignment_type: alignment.base().active_type(),
        diff_against: None,
    };

    let reference_line = alignment.rows().reference().map_or_else(
        || Line::from("No reference selected".fg(theme.theme.text_dim).italic()),
        |absolute_row| {
            let Some(projected_row) = alignment.view().project_absolute_row(absolute_row) else {
                return Line::from("No reference selected".fg(theme.theme.text_dim).italic());
            };
            let spans = format_row_view_spans(
                projected_row,
                &window.col_range,
                &theme.theme.sequence,
                no_diff_mode,
            );
            Line::from(spans)
        },
    );

    let consensus_bytes: Option<Vec<u8>> = window
        .col_range
        .clone()
        .map(|rel_col| {
            metrics
                .raw_summary_at(rel_col)
                .map(|summary| summary.consensus.unwrap_or(b' '))
        })
        .collect();

    let consensus_line = consensus_bytes.map_or_else(
        || Line::from("Calculating consensus...".fg(theme.theme.text_dim).italic()),
        |bytes| {
            let spans = format_row_spans(&bytes, &theme.theme.sequence, no_diff_mode);
            Line::from(spans)
        },
    );

    if shows_conservation_line(alignment) {
        let conservation_line = build_conservation_line(metrics, window, theme);
        vec![reference_line, consensus_line, conservation_line]
    } else {
        vec![reference_line, consensus_line]
    }
}

fn build_conservation_line(
    metrics: &ColumnStatsCache,
    window: &ViewportWindow,
    theme: &ThemeState,
) -> Line<'static> {
    let mut sparkline = String::with_capacity(window.col_range.len());

    for relative_col in window.col_range.clone() {
        let Some(summary) = metrics.raw_summary_at(relative_col) else {
            return Line::from(
                "Calculating conservation..."
                    .fg(theme.theme.text_dim)
                    .italic(),
            );
        };
        let spark = summary
            .conservation
            .filter(|value| value.is_finite())
            .map_or(" ", conservation_to_spark);
        sparkline.push_str(spark);
    }

    Line::from(sparkline).set_style(theme.styles.accent_alt)
}

#[cfg(test)]
mod tests {
    use ratatui::{buffer::Buffer, layout::Rect};

    use super::*;
    use crate::{
        core::{model::StatsView, stats_cache::StatsJobResult},
        ui::layout::{AlignmentHeaderLayout, AppLayout},
    };

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn alignment_model(sequences: Vec<libmsa::RawSequence>) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(sequences).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    fn metrics_with(
        view: StatsView,
        consensus: &[u8],
        conservation: Option<f32>,
    ) -> ColumnStatsCache {
        let mut cache = ColumnStatsCache::default();
        match view {
            StatsView::Raw => cache.init(consensus.len()),
            StatsView::Translated(frame) => {
                cache.init(consensus.len() * 3);
                let _ =
                    cache.translated_chunks_to_spawn(&(0..consensus.len()), frame, consensus.len());
            }
        }

        let summaries = consensus
            .iter()
            .enumerate()
            .map(|(position, &byte)| libmsa::ColumnSummary {
                position,
                consensus: Some(byte),
                conservation,
            })
            .collect();
        let generation = cache.generation;
        let chunk_idx = 0;
        let stored = cache.store(StatsJobResult {
            generation,
            chunk_idx,
            view,
            summaries: Ok(summaries),
        });
        assert!(stored);
        cache
    }

    fn render_consensus_pane_text(
        alignment: &AlignmentModel,
        metrics: &ColumnStatsCache,
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
            metrics,
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
        let metrics = metrics_with(StatsView::Raw, b"CATCATCATCATCATCAT", Some(1.0));

        insta::assert_snapshot!(
            "consensus_pane_raw",
            render_consensus_pane_text(&alignment, &metrics, Rect::new(0, 0, 100, 5))
        );
    }

    #[test]
    fn raw_consensus_no_reference_snapshot() {
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
        ]);
        let metrics = metrics_with(StatsView::Raw, b"CATCATCATCATCATCAT", Some(1.0));

        insta::assert_snapshot!(
            "consensus_pane_raw_no_reference",
            render_consensus_pane_text(&alignment, &metrics, Rect::new(0, 0, 100, 5))
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
        let metrics = metrics_with(
            StatsView::Translated(libmsa::ReadingFrame::Frame1),
            b"HHHHHH",
            Some(1.0),
        );

        insta::assert_snapshot!(
            "consensus_pane_translated",
            render_consensus_pane_text(&alignment, &metrics, Rect::new(0, 0, 100, 5))
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
        let metrics = metrics_with(
            StatsView::Translated(libmsa::ReadingFrame::Frame1),
            b"HHHHHH",
            Some(1.0),
        );

        insta::assert_snapshot!(
            "consensus_pane_translated_no_reference",
            render_consensus_pane_text(&alignment, &metrics, Rect::new(0, 0, 100, 5))
        );
    }

    #[test]
    fn generic_consensus_hides_conservation_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"ACDEACDEACDE"),
            raw("seq2", b"ACDEACDEACDE"),
        ]);
        alignment.set_reference(0).unwrap();
        let metrics = metrics_with(StatsView::Raw, b"ACDEACDEACDE", Some(1.0));

        insta::assert_snapshot!(
            "consensus_pane_generic_without_conservation",
            render_consensus_pane_text(&alignment, &metrics, Rect::new(0, 0, 100, 4))
        );
    }
}
