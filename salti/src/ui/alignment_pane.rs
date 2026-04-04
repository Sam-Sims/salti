use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::macros::vertical;
use ratatui::style::Styled;
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::{
    core::{
        codon::TranslatedDiffRange,
        model::{AlignmentModel, DiffMode},
        stats_cache::ColumnStatsCache,
        viewport::{Viewport, ViewportWindow},
    },
    ui::{
        layout::{AppLayout, PinnedSectionLayout, RULER_HEIGHT_ROWS, pinned_section_layout},
        rows::{RowRenderMode, format_row_spans, format_translated_row_spans, visible_bytes},
        ui_state::ThemeState,
    },
};

const SCROLLBAR_THUMB_WIDTH: usize = 3;
const SCROLLBAR_THUMB_MIN_WIDTH: usize = 1;

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
            let bytes = visible_bytes(projected_row, &window.col_range);
            let spans = format_row_spans(&bytes, &theme.theme.sequence, render_mode);
            Some(Line::from(spans))
        },
    );

    lines
}

fn render_sequence_rows(
    f: &mut Frame,
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    metrics: &ColumnStatsCache,
    area: Rect,
    theme: &ThemeState,
) {
    let lines = build_sequence_row_lines(alignment, window, metrics, area, theme);
    f.render_widget(Paragraph::new(lines).style(theme.styles.base_block), area);
}

fn render_scrollbar(
    f: &mut Frame,
    alignment: &AlignmentModel,
    viewport: &Viewport,
    window: &ViewportWindow,
    theme: &ThemeState,
    area: Rect,
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
        if let Some(cell) = f.buffer_mut().cell_mut((thumb_x, thumb_y)) {
            let track_colour = cell.fg;
            cell.set_char('▬');
            cell.set_fg(thumb_colour);
            cell.set_bg(track_colour);
        }
    }
}

fn add_number_to_ruler(
    number_line: &mut [Span<'static>],
    centre_pos: usize,
    number: usize,
    theme: &ThemeState,
) -> bool {
    let number_string = number.to_string();
    let number_length = number_string.len();
    let ruler_width = number_line.len();
    let start_idx = centre_pos
        .saturating_sub(number_length / 2)
        .min(ruler_width.saturating_sub(number_length));
    let left_padding = start_idx.saturating_sub(1);
    let right_padding = (start_idx + number_length + 1).min(ruler_width);

    if number_line[left_padding..right_padding]
        .iter()
        .any(|span| span.content.as_ref() != " ")
    {
        return false;
    }

    for (offset, digit) in number_string.chars().enumerate() {
        if let Some(cell) = number_line.get_mut(start_idx + offset) {
            *cell = digit.to_string().set_style(theme.styles.accent);
        }
    }

    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakMarker {
    Leading,
    Trailing,
}

fn break_positions(
    absolute_columns: &[usize],
    filtered_leading: bool,
    filtered_trailing: bool,
) -> Vec<(usize, BreakMarker)> {
    let width = absolute_columns.len();
    if width == 0 {
        return Vec::new();
    }

    let mut breaks = Vec::new();

    if filtered_leading {
        breaks.push((0, BreakMarker::Leading));
    }

    for (index, pair) in absolute_columns.windows(2).enumerate() {
        if pair[1] != pair[0] + 1 {
            breaks.push((index, BreakMarker::Trailing));
        }
    }

    if filtered_trailing {
        let last = width - 1;
        if !breaks.iter().any(|&(position, _)| position == last) {
            breaks.push((last, BreakMarker::Trailing));
        }
    }

    breaks
}

fn dense_break_marker_position(position: usize, marker: BreakMarker, width: usize) -> usize {
    match marker {
        BreakMarker::Leading => position,
        BreakMarker::Trailing => {
            if position + 1 < width {
                position + 1
            } else {
                position
            }
        }
    }
}

fn dense_break_spans(breaks: &[(usize, BreakMarker)], width: usize) -> Vec<(usize, usize)> {
    let marker_positions: Vec<usize> = breaks
        .iter()
        .map(|&(position, marker)| dense_break_marker_position(position, marker, width))
        .collect();
    let mut spans = Vec::new();
    let mut cluster_start = 0;

    while cluster_start < marker_positions.len() {
        let mut cluster_end = cluster_start + 1;
        while cluster_end < marker_positions.len()
            && marker_positions[cluster_end] <= marker_positions[cluster_end - 1] + 3
        {
            cluster_end += 1;
        }

        if cluster_end - cluster_start >= 2 {
            spans.push((
                marker_positions[cluster_start],
                marker_positions[cluster_end - 1],
            ));
        }

        cluster_start = cluster_end;
    }

    spans
}

fn run_start_positions(absolute_columns: &[usize]) -> Vec<usize> {
    let mut starts = Vec::new();
    if absolute_columns.is_empty() {
        return starts;
    }

    starts.push(0);
    for (index, pair) in absolute_columns.windows(2).enumerate() {
        if pair[1] != pair[0] + 1 {
            starts.push(index + 1);
        }
    }

    starts
}

fn build_ruler(
    absolute_columns: &[usize],
    filtered_leading: bool,
    filtered_trailing: bool,
    theme: &ThemeState,
) -> (Line<'static>, Line<'static>) {
    let width = absolute_columns.len();
    if width == 0 {
        return (Line::from(""), Line::from(""));
    }

    let mut number_line = vec![Span::raw(" "); width];
    let mut marker_line = vec![Span::raw(" "); width];
    let breaks = break_positions(absolute_columns, filtered_leading, filtered_trailing);
    let fragmented_view = !breaks.is_empty();
    let run_starts = fragmented_view.then(|| run_start_positions(absolute_columns));

    for (index, marker_span) in marker_line.iter_mut().enumerate() {
        let display_pos = absolute_columns[index] + 1;
        if display_pos == 1 || display_pos.is_multiple_of(5) {
            let is_major_tick = display_pos.is_multiple_of(10);
            *marker_span = if is_major_tick {
                "|".set_style(theme.styles.accent)
            } else {
                ".".set_style(theme.styles.text_dim)
            };

            let is_run_start = run_starts
                .as_ref()
                .is_some_and(|run_starts| run_starts.contains(&index));
            if is_major_tick || display_pos == 1 || is_run_start {
                let _ = add_number_to_ruler(&mut number_line, index, display_pos, theme);
            }
        }
    }

    let dense_spans = dense_break_spans(&breaks, width);

    for (position, marker) in breaks {
        let marker_position = dense_break_marker_position(position, marker, width);
        if dense_spans
            .iter()
            .any(|&(start, end)| start <= marker_position && marker_position <= end)
        {
            continue;
        }

        let symbol = match marker {
            BreakMarker::Leading => "‹",
            BreakMarker::Trailing => "›",
        };
        marker_line[position] = symbol.set_style(theme.styles.warning);
    }

    for (start, end) in dense_spans {
        for marker in marker_line.iter_mut().take(end + 1).skip(start) {
            *marker = "~".set_style(theme.styles.warning);
        }
    }

    (Line::from(number_line), Line::from(marker_line))
}

fn render_ruler(
    f: &mut Frame,
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    area: Rect,
    theme: &ThemeState,
) {
    let absolute_columns: Vec<usize> = window
        .col_range
        .clone()
        .filter_map(|relative_col| alignment.view().absolute_column_id(relative_col))
        .collect();
    let filtered_leading = window.col_range.start == 0
        && alignment
            .view()
            .absolute_column_id(0)
            .is_some_and(|first| first > 0);
    let filtered_trailing = window.col_range.end >= alignment.view().column_count()
        && alignment.base().column_count() > 0
        && alignment
            .view()
            .absolute_column_id(alignment.view().column_count().saturating_sub(1))
            .is_some_and(|last| last < alignment.base().column_count() - 1);
    let (number_line, marker_line) = build_ruler(
        &absolute_columns,
        filtered_leading,
        filtered_trailing,
        theme,
    );
    f.render_widget(
        Paragraph::new(vec![number_line, marker_line]).style(theme.styles.base_block),
        area,
    );
}

pub fn render_alignment_pane(
    f: &mut Frame,
    layout: &AppLayout,
    alignment: &AlignmentModel,
    viewport: &Viewport,
    metrics: &ColumnStatsCache,
    theme: &ThemeState,
) {
    let block = Block::bordered()
        .title(Line::from("Alignment".set_style(theme.styles.accent)))
        .border_style(theme.styles.border)
        .style(theme.styles.base_block)
        .merge_borders(MergeStrategy::Exact);
    let inner_area = block.inner(layout.alignment_pane);
    f.render_widget(block, layout.alignment_pane);

    let [ruler_area, sequence_rows_area] = inner_area.layout(&vertical![==RULER_HEIGHT_ROWS, *=1]);
    let window = viewport.window();

    render_ruler(f, alignment, &window, ruler_area, theme);
    render_sequence_rows(f, alignment, &window, metrics, sequence_rows_area, theme);
    render_scrollbar(
        f,
        alignment,
        viewport,
        &window,
        theme,
        layout.alignment_pane,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::StatsView;
    use crate::core::stats_cache::StatsJobResult;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

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
        let backend = TestBackend::new(area.width, area.height);
        let mut terminal = Terminal::new(backend).unwrap();
        let layout = AppLayout::new(area);
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

        terminal
            .draw(|frame| {
                render_alignment_pane(
                    frame,
                    &layout,
                    alignment,
                    &viewport,
                    stats_cache,
                    &ThemeState::default(),
                );
            })
            .unwrap();

        buffer_text(terminal.backend().buffer(), layout.alignment_pane)
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
        alignment.set_translation(Some(libmsa::ReadingFrame::Frame1)).unwrap();

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
        alignment.set_translation(Some(libmsa::ReadingFrame::Frame1)).unwrap();
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
    fn alignment_pane_raw_diff_reference_without_reference_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATGATCATCATCAT"),
            raw("seq3", b"CATCATCATCATGATCAT"),
        ]);
        alignment.diff_mode = DiffMode::Reference;

        insta::assert_snapshot!(
            "alignment_pane_raw_diff_reference_without_reference",
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
    fn alignment_pane_raw_diff_consensus_loading_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATGATCATCATCAT"),
            raw("seq3", b"CATCATCATCATGATCAT"),
        ]);
        alignment.diff_mode = DiffMode::Consensus;
        let mut metrics = ColumnStatsCache::default();
        metrics.init(alignment.view().column_count());

        insta::assert_snapshot!(
            "alignment_pane_raw_diff_consensus_loading",
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

    #[test]
    fn alignment_pane_dense_fragmented_ruler_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CA-CA-CA-CA-CA-CA-CA-CA"),
            raw("seq2", b"CA-CA-CA-CA-CA-CA-CA-CA"),
            raw("seq3", b"CA-CA-CA-CA-CA-CA-CA-CA"),
        ]);
        alignment.set_gap_filter(Some(0.5)).unwrap();

        insta::assert_snapshot!(
            "alignment_pane_dense_fragmented_ruler",
            render_alignment_pane_text(
                &alignment,
                &ColumnStatsCache::default(),
                Rect::new(0, 0, 100, 12),
                0,
                0,
            )
        );
    }
}
