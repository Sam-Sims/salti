use crate::{
    core::{model::AlignmentModel, stats_cache::ColumnStatsCache, viewport::ViewportWindow},
    ui::{
        layout::AppLayout,
        rows::{
            RowRenderMode, TranslatedByteRange, format_row_spans,
            format_translated_byte_range_spans, format_translated_row_spans, visible_bytes,
            visible_protein_range,
        },
        ui_state::ThemeState,
    },
};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Styled, Stylize};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

const CONSERVATION_SPARK_STRS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

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
    window: &ViewportWindow,
    theme: &ThemeState,
) -> Line<'static> {
    let Some(translated) = alignment.translated_view() else {
        return Line::from("No reference selected".fg(theme.theme.text_dim).italic());
    };
    let frame = alignment
        .translation()
        .expect("translated view requires an active frame");
    let nucleotide_len = alignment.view().column_count();

    alignment.rows().reference().map_or_else(
        || Line::from("No reference selected".fg(theme.theme.text_dim).italic()),
        |absolute_row| {
            let Some(sequence) = translated.project_absolute_row(absolute_row) else {
                return Line::from("No reference selected".fg(theme.theme.text_dim).italic());
            };
            let spans = format_translated_row_spans(
                sequence,
                &window.col_range,
                nucleotide_len,
                frame,
                &theme.theme.sequence,
                None,
            );
            Line::from(spans)
        },
    )
}

fn translated_consensus_line(
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    theme: &ThemeState,
) -> Line<'static> {
    let Some(_translated) = alignment.translated_view() else {
        return Line::from("Calculating consensus...".fg(theme.theme.text_dim).italic());
    };
    let frame = alignment
        .translation()
        .expect("translated view requires an active frame");
    let nucleotide_len = alignment.view().column_count();
    let Some(protein_range) = visible_protein_range(&window.col_range, frame, nucleotide_len)
    else {
        return blank_line(window.col_range.len());
    };

    let consensus_bytes: Option<Vec<u8>> = protein_range
        .clone()
        .map(|protein_col| {
            metrics
                .translated_summary_at(frame, protein_col)
                .map(|summary| summary.consensus.unwrap_or(b' '))
        })
        .collect();
    let Some(consensus_bytes) = consensus_bytes else {
        return Line::from("Calculating consensus...".fg(theme.theme.text_dim).italic());
    };
    let spans = format_translated_byte_range_spans(
        TranslatedByteRange::new(protein_range.start, &consensus_bytes),
        &window.col_range,
        nucleotide_len,
        frame,
        &theme.theme.sequence,
        None,
    );
    Line::from(spans)
}

fn translated_conservation_line(
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    theme: &ThemeState,
) -> Line<'static> {
    let Some(frame) = alignment.translation() else {
        return Line::from(
            "Calculating conservation..."
                .fg(theme.theme.text_dim)
                .italic(),
        );
    };
    let nucleotide_len = alignment.view().column_count();
    let width = window.col_range.len();
    let mut spans = vec![ratatui::text::Span::styled(" ", theme.styles.accent_alt); width];

    let Some(protein_range) = visible_protein_range(&window.col_range, frame, nucleotide_len)
    else {
        return Line::from(spans);
    };

    for protein_col in protein_range {
        let Some(summary) = metrics.translated_summary_at(frame, protein_col) else {
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
        let nuc_start = frame.offset() + protein_col * 3;

        for absolute_col in nuc_start..=nuc_start + 2 {
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
    if alignment.translation().is_some() {
        return vec![
            translated_reference_line(alignment, window, theme),
            translated_consensus_line(alignment, window, metrics, theme),
            translated_conservation_line(alignment, window, metrics, theme),
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
            let bytes = visible_bytes(projected_row, &window.col_range);
            let spans = format_row_spans(&bytes, &theme.theme.sequence, no_diff_mode);
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

fn render_consensus_alignment_pane(
    f: &mut Frame,
    area: Rect,
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    theme: &ThemeState,
) {
    let block = Block::bordered()
        .border_style(theme.styles.border)
        .style(theme.styles.base_block)
        .merge_borders(MergeStrategy::Exact);
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let lines = consensus_alignment_lines(alignment, window, metrics, theme);
    f.render_widget(
        Paragraph::new(lines).style(theme.styles.base_block),
        inner_area,
    );
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

fn render_consensus_sequence_id_pane(
    f: &mut Frame,
    area: Rect,
    alignment: &AlignmentModel,
    theme: &ThemeState,
) {
    let block = Block::bordered()
        .border_style(theme.styles.border)
        .style(theme.styles.base_block)
        .merge_borders(MergeStrategy::Exact);
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let lines = if shows_conservation_line(alignment) {
        vec![
            Line::from("Reference Sequence:".set_style(theme.styles.accent)),
            Line::from("Consensus Sequence:".set_style(theme.styles.accent)),
            Line::from("Conservation:".set_style(theme.styles.accent)),
        ]
    } else {
        vec![
            Line::from("Reference Sequence:".set_style(theme.styles.accent)),
            Line::from("Consensus Sequence:".set_style(theme.styles.accent)),
        ]
    };

    f.render_widget(
        Paragraph::new(lines).style(theme.styles.base_block),
        inner_area,
    );
}

pub fn render_consensus_pane(
    f: &mut Frame,
    layout: &AppLayout,
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    theme: &ThemeState,
) {
    render_consensus_sequence_id_pane(f, layout.consensus_sequence_id_pane, alignment, theme);
    render_consensus_alignment_pane(
        f,
        layout.consensus_alignment_pane,
        alignment,
        window,
        metrics,
        theme,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::StatsView;
    use crate::core::stats_cache::StatsJobResult;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
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
                gap_fraction: 0.0,
            })
            .collect();
        let view = view;
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

    #[test]
    fn translated_consensus_lines_use_codon_spread_rendering() {
        let alignment =
            libmsa::Alignment::new(vec![raw("ref", b"ATGAAATTT"), raw("row", b"ATGAAATTT")])
                .expect("alignment should be valid");
        let mut alignment =
            AlignmentModel::new(alignment).expect("alignment model should be created");
        alignment.set_reference(0).expect("reference should be set");
        alignment
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .expect("translation should succeed");

        let window = ViewportWindow {
            row_range: 0..alignment.view().row_count(),
            col_range: 0..alignment.view().column_count(),
            name_range: 0..0,
        };
        let lines = consensus_alignment_lines(
            &alignment,
            &window,
            &metrics_with(
                StatsView::Translated(libmsa::ReadingFrame::Frame1),
                b"MKF",
                Some(1.0),
            ),
            &ThemeState::default(),
        );

        assert_eq!(lines.len(), 3);
        assert_eq!(line_text(&lines[0]), " M  K  F ");
        assert_eq!(line_text(&lines[1]), " M  K  F ");
        assert_eq!(line_text(&lines[2]), "█████████");
    }

    #[test]
    fn translated_mode_keeps_conservation_label() {
        let alignment =
            libmsa::Alignment::new(vec![raw("ref", b"ATGAAATTT"), raw("row", b"ATGAAATTT")])
                .expect("alignment should be valid");
        let mut alignment =
            AlignmentModel::new(alignment).expect("alignment model should be created");
        alignment
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .expect("translation should succeed");

        assert!(shows_conservation_line(&alignment));
    }

    #[test]
    fn raw_consensus_lines_keep_conservation_row() {
        let alignment = libmsa::Alignment::new(vec![raw("ref", b"ACGT"), raw("row", b"ACGT")])
            .expect("alignment should be valid");
        let mut alignment =
            AlignmentModel::new(alignment).expect("alignment model should be created");
        alignment.set_reference(0).expect("reference should be set");

        let window = ViewportWindow {
            row_range: 0..alignment.view().row_count(),
            col_range: 0..alignment.view().column_count(),
            name_range: 0..0,
        };
        let lines = consensus_alignment_lines(
            &alignment,
            &window,
            &metrics_with(StatsView::Raw, b"ACGT", Some(1.0)),
            &ThemeState::default(),
        );

        assert_eq!(lines.len(), 3);
        assert_eq!(line_text(&lines[0]), "ACGT");
        assert_eq!(line_text(&lines[1]), "ACGT");
    }
}
