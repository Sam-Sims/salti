use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Styled,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::{
    core::{model::AlignmentModel, viewport::ViewportWindow},
    ui::ui_state::ThemeState,
};

pub(crate) struct Ruler<'a> {
    pub(crate) alignment: &'a AlignmentModel,
    pub(crate) window: &'a ViewportWindow,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for Ruler<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let absolute_columns: Vec<usize> = self
            .window
            .col_range
            .clone()
            .filter_map(|relative_col| self.alignment.view().absolute_column_id(relative_col))
            .collect();
        let filtered_leading = self.window.col_range.start == 0
            && self
                .alignment
                .view()
                .absolute_column_id(0)
                .is_some_and(|first| first > 0);
        let filtered_trailing = self.window.col_range.end >= self.alignment.view().column_count()
            && self.alignment.base().column_count() > 0
            && self
                .alignment
                .view()
                .absolute_column_id(self.alignment.view().column_count().saturating_sub(1))
                .is_some_and(|last| last < self.alignment.base().column_count() - 1);
        let (number_line, marker_line) = build_ruler(
            &absolute_columns,
            filtered_leading,
            filtered_trailing,
            self.theme,
        );
        Paragraph::new(vec![number_line, marker_line])
            .style(self.theme.styles.base_block)
            .render(area, buf);
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
    let mut run_starts =
        fragmented_view.then(|| run_start_positions(absolute_columns).into_iter().peekable());

    for (index, marker_span) in marker_line.iter_mut().enumerate() {
        let is_run_start = run_starts.as_mut().is_some_and(|starts| {
            while starts.peek().is_some_and(|&start| start < index) {
                let _ = starts.next();
            }
            matches!(starts.peek(), Some(&start) if start == index)
        });
        let display_pos = absolute_columns[index] + 1;
        if display_pos == 1 || display_pos.is_multiple_of(5) {
            let is_major_tick = display_pos.is_multiple_of(10);
            *marker_span = if is_major_tick {
                "|".set_style(theme.styles.accent)
            } else {
                ".".set_style(theme.styles.text_dim)
            };

            if is_run_start {
                let _ = run_starts.as_mut().and_then(Iterator::next);
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn basic_ruler_marks_positions() {
        let theme = ThemeState::default();
        let absolute_columns: Vec<usize> = (0..18).collect();

        let (number_line, marker_line) = build_ruler(&absolute_columns, false, false, &theme);

        assert_eq!(line_text(&number_line), "1       10        ");
        assert_eq!(line_text(&marker_line), ".   .    |    .   ");
    }

    #[test]
    fn fragmented_ruler_marks_break() {
        let theme = ThemeState::default();
        let absolute_columns = [0, 1, 2, 6, 7];

        let (number_line, marker_line) = build_ruler(&absolute_columns, false, false, &theme);

        assert_eq!(line_text(&number_line), "1    ");
        assert_eq!(line_text(&marker_line), ". ›  ");
    }

    #[test]
    fn dense_fragmented_ruler_uses_span() {
        let theme = ThemeState::default();
        let absolute_columns = [0, 2, 4, 6, 8, 10];

        let (number_line, marker_line) = build_ruler(&absolute_columns, false, false, &theme);

        assert_eq!(line_text(&number_line), "1 5   ");
        assert_eq!(line_text(&marker_line), ".~~~~~");
    }

    #[test]
    fn filtered_ruler_marks_edges() {
        let theme = ThemeState::default();
        let absolute_columns = [2, 3, 4, 5, 6];

        let (number_line, marker_line) = build_ruler(&absolute_columns, true, true, &theme);

        assert_eq!(line_text(&number_line), "     ");
        assert_eq!(line_text(&marker_line), "‹ . ›");
    }
}
