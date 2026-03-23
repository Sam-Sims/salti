use crate::{
    core::{
        model::{AlignmentModel, DiffMode},
        stats_cache::ColumnStatsCache,
        viewport::{Viewport, ViewportWindow},
    },
    ui::{
        layout::{AppLayout, RULER_HEIGHT_ROWS, pinned_section_layout},
        rows::{
            RowRenderMode, TranslatedDiffRange, format_row_spans, format_translated_row_spans,
            visible_bytes, visible_protein_range,
        },
        ui_state::ThemeState,
    },
};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::macros::vertical;
use ratatui::style::Styled;
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

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

    if let Some(translated) = alignment.translated_view() {
        let frame = alignment
            .translation()
            .expect("translated view requires an active frame");
        let nucleotide_len = alignment.view().column_count();
        let protein_range = visible_protein_range(&window.col_range, frame, nucleotide_len);
        let reference_bytes: Option<Vec<u8>> = protein_range.clone().and_then(|protein_range| {
            alignment
                .rows()
                .reference()
                .and_then(|abs_row| translated.project_absolute_row(abs_row))
                .map(|sequence| {
                    sequence
                        .bytes_range(protein_range)
                        .expect("visible protein range must fit the translated view")
                        .map(|(_, byte)| byte)
                        .collect()
                })
        });
        let consensus_bytes: Option<Vec<u8>> = protein_range.clone().and_then(|protein_range| {
            protein_range
                .clone()
                .map(|protein_col| {
                    metrics
                        .translated_summary_at(frame, protein_col)
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

        for &absolute_row in alignment
            .rows()
            .pinned()
            .iter()
            .take(band_layout.pinned_rendered)
        {
            let Some(sequence) = translated.project_absolute_row(absolute_row) else {
                continue;
            };
            let spans = format_translated_row_spans(
                sequence,
                &window.col_range,
                nucleotide_len,
                frame,
                &theme.theme.sequence,
                diff_against,
            );
            lines.push(Line::from(spans));
        }

        if band_layout.divider_height == 1 {
            lines.push(Line::from(
                "─"
                    .repeat(area.width as usize)
                    .set_style(theme.styles.border),
            ));
        }

        for relative_row in window.row_range.clone() {
            let Some(absolute_row) = alignment.view().absolute_row_id(relative_row) else {
                continue;
            };
            let Some(sequence) = translated.sequence_by_absolute(absolute_row) else {
                continue;
            };
            let spans = format_translated_row_spans(
                sequence,
                &window.col_range,
                nucleotide_len,
                frame,
                &theme.theme.sequence,
                diff_against,
            );
            lines.push(Line::from(spans));
        }

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

    for &absolute_row in alignment
        .rows()
        .pinned()
        .iter()
        .take(band_layout.pinned_rendered)
    {
        let Some(projected_row) = alignment.view().project_absolute_row(absolute_row) else {
            continue;
        };
        let bytes = visible_bytes(projected_row, &window.col_range);
        let spans = format_row_spans(&bytes, &theme.theme.sequence, render_mode);
        lines.push(Line::from(spans));
    }

    if band_layout.divider_height == 1 {
        lines.push(Line::from(
            "─"
                .repeat(area.width as usize)
                .set_style(theme.styles.border),
        ));
    }

    for relative_row in window.row_range.clone() {
        let Some(sequence) = alignment.view().sequence(relative_row) else {
            continue;
        };
        let bytes = visible_bytes(sequence, &window.col_range);
        let spans = format_row_spans(&bytes, &theme.theme.sequence, render_mode);
        lines.push(Line::from(spans));
    }

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
) {
    let number_string = number.to_string();
    let number_length = number_string.len();
    let ruler_width = number_line.len();
    let start_idx = centre_pos
        .saturating_sub(number_length / 2)
        .min(ruler_width.saturating_sub(number_length));

    for (offset, digit) in number_string.chars().enumerate() {
        if let Some(cell) = number_line.get_mut(start_idx + offset) {
            *cell = digit.to_string().set_style(theme.styles.accent);
        }
    }
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

    for (index, marker_span) in marker_line.iter_mut().enumerate() {
        let display_pos = absolute_columns[index] + 1;
        if display_pos == 1 || display_pos.is_multiple_of(5) {
            let is_major_tick = display_pos.is_multiple_of(10);
            *marker_span = if is_major_tick {
                "|".set_style(theme.styles.accent)
            } else {
                ".".set_style(theme.styles.text_dim)
            };

            if is_major_tick || display_pos == 1 {
                add_number_to_ruler(&mut number_line, index, display_pos, theme);
            }
        }
    }

    let breaks = break_positions(absolute_columns, filtered_leading, filtered_trailing);
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
        for position in start..=end {
            marker_line[position] = "~".set_style(theme.styles.warning);
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