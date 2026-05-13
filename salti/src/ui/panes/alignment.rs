use ratatui::{
    buffer::Buffer,
    layout::Rect,
    macros::vertical,
    style::Styled,
    symbols::merge::MergeStrategy,
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

use crate::{
    core::{
        codon::TranslatedDiffRange,
        gff::Gff,
        model::{AlignmentModel, DiffMode},
        stats_cache::ColumnStatsCache,
        viewport::{Viewport, ViewportWindow},
    },
    ui::{
        layout::{AlignmentHeaderLayout, PinnedSectionLayout, pinned_section_layout},
        panes::{local_feature_track::LocalFeatureTrack, ruler::Ruler},
        rows::{RowRenderMode, format_row_view_spans, format_translated_row_spans, visible_bytes},
        ui_state::ThemeState,
    },
};

const SCROLLBAR_THUMB_WIDTH: usize = 3;
const SCROLLBAR_THUMB_MIN_WIDTH: usize = 1;

pub(crate) struct AlignmentPane<'a> {
    pub(crate) alignment: &'a AlignmentModel,
    pub(crate) viewport: &'a Viewport,
    pub(crate) metrics: &'a ColumnStatsCache,
    pub(crate) gff: Option<&'a Gff>,
    pub(crate) header: AlignmentHeaderLayout,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for AlignmentPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_style(self.theme.styles.border)
            .style(self.theme.styles.base_block)
            .merge_borders(MergeStrategy::Exact);
        let inner_area = block.inner(area);
        block.render(area, buf);

        let [local_feature_area, ruler_area, sequence_rows_area] = inner_area.layout(&vertical![
            ==self.header.local_feature_rows,
            ==self.header.ruler_rows,
            *=1
        ]);
        let window = self.viewport.window();

        if let Some(gff) = self.gff {
            LocalFeatureTrack {
                gff,
                alignment: self.alignment,
                window: &window,
                theme: self.theme,
            }
            .render(local_feature_area, buf);
        }
        Ruler {
            alignment: self.alignment,
            window: &window,
            theme: self.theme,
        }
        .render(ruler_area, buf);
        render_sequence_rows(
            self.alignment,
            &window,
            self.metrics,
            sequence_rows_area,
            self.theme,
            buf,
        );
        render_scrollbar(
            self.alignment,
            self.viewport,
            &window,
            self.theme,
            area,
            buf,
        );
    }
}

fn raw_render_mode<'a>(
    alignment: &AlignmentModel,
    reference_bytes: Option<&'a [u8]>,
    consensus_bytes: Option<&'a [u8]>,
) -> RowRenderMode<'a> {
    let diff_against = match alignment.diff_mode {
        DiffMode::Off => None,
        DiffMode::Reference => reference_bytes,
        DiffMode::Consensus => consensus_bytes,
    };

    RowRenderMode {
        alignment_type: alignment.base().active_type(),
        diff_against,
    }
}

fn translated_diff_range<'a>(
    diff_mode: DiffMode,
    protein_range_start: usize,
    reference_bytes: Option<&'a [u8]>,
    consensus_bytes: Option<&'a [u8]>,
) -> Option<TranslatedDiffRange<'a>> {
    match diff_mode {
        DiffMode::Off => None,
        DiffMode::Reference => {
            reference_bytes.map(|bytes| TranslatedDiffRange::new(protein_range_start, bytes))
        }
        DiffMode::Consensus => {
            consensus_bytes.map(|bytes| TranslatedDiffRange::new(protein_range_start, bytes))
        }
    }
}

fn emit_band_rows(
    lines: &mut Vec<Line<'static>>,
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    band_layout: &PinnedSectionLayout,
    area_width: u16,
    theme: &ThemeState,
    render_row: &mut dyn FnMut(usize) -> Option<Line<'static>>,
) {
    for &absolute_row in alignment
        .rows()
        .pinned()
        .iter()
        .take(band_layout.pinned_rendered)
    {
        if let Some(line) = render_row(absolute_row) {
            lines.push(line);
        }
    }

    if band_layout.divider_height == 1 {
        lines.push(Line::from(
            "─"
                .repeat(area_width as usize)
                .set_style(theme.styles.border),
        ));
    }

    for relative_row in window.row_range.clone() {
        let Some(absolute_row) = alignment.view().absolute_row_id(relative_row) else {
            continue;
        };
        if let Some(line) = render_row(absolute_row) {
            lines.push(line);
        }
    }
}

fn build_sequence_row_lines(
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    area: Rect,
    theme: &ThemeState,
) -> Vec<Line<'static>> {
    let band_layout = pinned_section_layout(alignment.rows().pinned().len(), area.height as usize);
    let mut lines = Vec::with_capacity(
        band_layout.pinned_rendered + band_layout.divider_height + window.row_range.len(),
    );

    if let Some(overlay) = alignment.translation_overlay()
        && let Some(translated) = alignment.translated_view()
    {
        let protein_range = overlay.visible_protein_range(&window.col_range);
        let reference_bytes: Option<Vec<u8>> = protein_range.clone().and_then(|protein_range| {
            alignment
                .rows()
                .reference()
                .and_then(|abs_row| translated.project_absolute_row(abs_row))
                .and_then(|sequence| {
                    let bytes = sequence.bytes_range(protein_range).ok()?;
                    Some(bytes.map(|(_, byte)| byte).collect())
                })
        });
        let consensus_bytes: Option<Vec<u8>> = protein_range.clone().and_then(|protein_range| {
            protein_range
                .clone()
                .map(|protein_col: usize| {
                    metrics
                        .translated_summary_at(overlay.frame, protein_col)
                        .map(|summary| summary.consensus.unwrap_or(b' '))
                })
                .collect()
        });
        let diff_against = protein_range.as_ref().and_then(|protein_range| {
            translated_diff_range(
                alignment.diff_mode,
                protein_range.start,
                reference_bytes.as_deref(),
                consensus_bytes.as_deref(),
            )
        });

        emit_band_rows(
            &mut lines,
            alignment,
            window,
            &band_layout,
            area.width,
            theme,
            &mut |absolute_row| {
                let sequence = translated.project_absolute_row(absolute_row)?;
                let spans = format_translated_row_spans(
                    sequence,
                    &window.col_range,
                    &overlay,
                    &theme.theme.sequence,
                    diff_against,
                );
                Some(Line::from(spans))
            },
        );

        return lines;
    }

    let reference_bytes: Option<Vec<u8>> = alignment
        .rows()
        .reference()
        .and_then(|abs_row| alignment.view().project_absolute_row(abs_row))
        .map(|sequence| visible_bytes(sequence, &window.col_range));
    let consensus_bytes: Option<Vec<u8>> = window
        .col_range
        .clone()
        .map(|relative_col| {
            metrics
                .raw_summary_at(relative_col)
                .map(|summary| summary.consensus.unwrap_or(b' '))
        })
        .collect();
    let render_mode = raw_render_mode(
        alignment,
        reference_bytes.as_deref(),
        consensus_bytes.as_deref(),
    );

    emit_band_rows(
        &mut lines,
        alignment,
        window,
        &band_layout,
        area.width,
        theme,
        &mut |absolute_row| {
            let projected_row = alignment.view().project_absolute_row(absolute_row)?;
            let spans = format_row_view_spans(
                projected_row,
                &window.col_range,
                &theme.theme.sequence,
                render_mode,
            );
            Some(Line::from(spans))
        },
    );

    lines
}

fn render_sequence_rows(
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    area: Rect,
    theme: &ThemeState,
    buf: &mut Buffer,
) {
    let lines = build_sequence_row_lines(alignment, window, metrics, area, theme);
    Paragraph::new(lines)
        .style(theme.styles.base_block)
        .render(area, buf);
}

fn render_scrollbar(
    alignment: &AlignmentModel,
    viewport: &Viewport,
    window: &ViewportWindow,
    theme: &ThemeState,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.width < 2 || area.height == 0 {
        return;
    }

    let total_columns = alignment.view().column_count();
    let visible_columns = window.col_range.len();
    if total_columns <= visible_columns {
        return;
    }

    let width = area.width.saturating_sub(2) as usize;
    let max_index = total_columns.saturating_sub(1);
    let col_offset = viewport.window().col_range.start;
    let percent = col_offset
        .saturating_mul(100)
        .checked_div(max_index)
        .unwrap_or(0);
    let track_max = width.saturating_sub(1);
    let thumb_index = if track_max == 0 {
        0
    } else {
        (percent * track_max) / 100
    };
    let scrollbar_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(1),
        width: area.width.saturating_sub(2),
        height: 1,
    };
    let thumb_width = if SCROLLBAR_THUMB_WIDTH <= width {
        SCROLLBAR_THUMB_WIDTH
    } else {
        SCROLLBAR_THUMB_MIN_WIDTH
    };
    let thumb_start = thumb_index.saturating_sub(thumb_width / 2);
    let thumb_end = (thumb_start + thumb_width).min(width);
    let thumb_y = scrollbar_area.y;
    let thumb_colour = theme.theme.accent_alt;

    for offset in thumb_start..thumb_end {
        let thumb_x = scrollbar_area.x + offset as u16;
        if let Some(cell) = buf.cell_mut((thumb_x, thumb_y)) {
            let track_colour = cell.fg;
            cell.set_char('▬');
            cell.set_fg(thumb_colour);
            cell.set_bg(track_colour);
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;

    use super::*;
    use crate::{
        core::{
            gff::{Feature, FeatureType, Gff, Strand},
            model::StatsView,
            stats_cache::StatsJobResult,
        },
        ui::layout::AppLayout,
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

    fn render_alignment_pane_text(
        alignment: &AlignmentModel,
        stats_cache: &ColumnStatsCache,
        area: Rect,
        row_offset: usize,
        col_offset: usize,
    ) -> String {
        render_alignment_pane_text_with_gff(
            alignment,
            stats_cache,
            None,
            area,
            row_offset,
            col_offset,
        )
    }

    fn render_alignment_pane_text_with_gff(
        alignment: &AlignmentModel,
        stats_cache: &ColumnStatsCache,
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
            metrics: stats_cache,
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
            render_alignment_pane_text(
                &alignment,
                &ColumnStatsCache::default(),
                Rect::new(0, 0, 100, 12),
                0,
                0,
            )
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
                &ColumnStatsCache::default(),
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
                &ColumnStatsCache::default(),
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
            render_alignment_pane_text(
                &alignment,
                &ColumnStatsCache::default(),
                Rect::new(0, 0, 100, 12),
                0,
                0,
            )
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
            render_alignment_pane_text(
                &alignment,
                &ColumnStatsCache::default(),
                Rect::new(0, 0, 100, 12),
                0,
                0,
            )
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
            render_alignment_pane_text(
                &alignment,
                &ColumnStatsCache::default(),
                Rect::new(0, 0, 100, 12),
                0,
                0,
            )
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
            render_alignment_pane_text(
                &alignment,
                &ColumnStatsCache::default(),
                Rect::new(0, 0, 100, 12),
                0,
                0,
            )
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
        let metrics = metrics_with(StatsView::Raw, b"CATCATCATCATCATCAT", Some(1.0));

        insta::assert_snapshot!(
            "alignment_pane_raw_diff_consensus",
            render_alignment_pane_text(&alignment, &metrics, Rect::new(0, 0, 100, 12), 0, 0,)
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
            render_alignment_pane_text(
                &alignment,
                &ColumnStatsCache::default(),
                Rect::new(0, 0, 60, 12),
                0,
                10,
            )
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
            render_alignment_pane_text(
                &alignment,
                &ColumnStatsCache::default(),
                Rect::new(0, 0, 100, 10),
                2,
                0,
            )
        );
    }
}
