use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Styled,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::{
    core::session::Session,
    ui::{layout::Window, ui_state::ThemeState},
};

pub(crate) struct Ruler<'a> {
    pub(crate) session: &'a Session,
    pub(crate) window: &'a Window,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for Ruler<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let cols = self.session.layout().columns();
        let (Some(&first), Some(&last)) = (cols.first(), cols.last()) else {
            return;
        };
        let shown = self.window.columns.clone();
        let filtered_leading = shown.start == 0 && first > 0;
        let filtered_trailing = shown.end == cols.len() && last < self.session.grid().width() - 1;

        let (number_line, marker_line) = build_ruler(
            &cols[shown],
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
    use std::ops::Range;

    use BreakMarker::{Leading, Trailing};
    use rstest::rstest;

    use super::*;
    use crate::{
        core::session::Position,
        test_utils::{buffer_text, session},
    };

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[rstest]
    #[case::contiguous(&(0..18).collect::<Vec<_>>(), false, false, "1       10        ", ".   .    |    .   ")]
    #[case::one_break(&[0, 1, 2, 6, 7], false, false, "1    ", ". ›  ")]
    #[case::close_breaks_become_span(&[0, 2, 4, 6, 8, 10], false, false, "1 5   ", ".~~~~~")]
    #[case::filtered_edges(&[2, 3, 4, 5, 6], true, true, "     ", "‹ . ›")]
    #[case::number_at_run_start(&[0, 1, 2, 14, 15, 16, 17, 18, 19], false, false, "1 15   20", ". ›.    |")]
    #[case::lone_break_beside_span(&[0, 2, 4, 10, 11, 12, 13, 14, 20], false, false, "1 5      ", ".~~~   › ")]
    #[case::crowded_numbers_dropped(&[0, 4, 9], false, false, "1  ", ".~~")]
    fn build_ruler_works(
        #[case] columns: &[usize],
        #[case] filtered_leading: bool,
        #[case] filtered_trailing: bool,
        #[case] numbers: &str,
        #[case] markers: &str,
    ) {
        let (number_line, marker_line) = build_ruler(
            columns,
            filtered_leading,
            filtered_trailing,
            &ThemeState::default(),
        );

        assert_eq!(
            (
                line_text(&number_line).as_str(),
                line_text(&marker_line).as_str()
            ),
            (numbers, markers)
        );
    }

    #[rstest]
    #[case::gap(&[0, 1, 5, 6], false, false, &[(1, Trailing)])]
    #[case::leading(&[3, 4], true, false, &[(0, Leading)])]
    #[case::trailing(&[0, 1], false, true, &[(1, Trailing)])]
    #[case::gap_and_trailing(&[0, 1, 5], false, true, &[(1, Trailing), (2, Trailing)])]
    fn break_positions_works(
        #[case] columns: &[usize],
        #[case] filtered_leading: bool,
        #[case] filtered_trailing: bool,
        #[case] expected: &[(usize, BreakMarker)],
    ) {
        assert_eq!(
            break_positions(columns, filtered_leading, filtered_trailing),
            expected
        );
    }

    #[rstest]
    #[case::contiguous(&[0, 1, 2], false)]
    #[case::no_columns(&[], true)]
    fn break_positions_is_empty(#[case] columns: &[usize], #[case] filtered_trailing: bool) {
        assert_eq!(break_positions(columns, false, filtered_trailing), []);
    }

    #[rstest]
    #[case::within_three(&[(0, Trailing), (3, Trailing)], &[(1, 4)])]
    #[case::leading_marks_own_column(&[(0, Leading), (1, Trailing)], &[(0, 2)])]
    #[case::trailing_at_end_stays(&[(8, Trailing), (9, Trailing)], &[(9, 9)])]
    #[case::two_clusters(&[(0, Trailing), (1, Trailing), (6, Trailing), (7, Trailing)], &[(1, 2), (7, 8)])]
    fn dense_break_spans_works(
        #[case] breaks: &[(usize, BreakMarker)],
        #[case] expected: &[(usize, usize)],
    ) {
        assert_eq!(dense_break_spans(breaks, 10), expected);
    }

    #[rstest]
    #[case::single(&[(0, Trailing)])]
    #[case::too_far_apart(&[(0, Trailing), (4, Trailing)])]
    fn dense_break_spans_is_empty(#[case] breaks: &[(usize, BreakMarker)]) {
        assert_eq!(dense_break_spans(breaks, 10), []);
    }

    #[rstest]
    #[case::whole_window(b"-AAAAAA-", 0..6, "‹  . ›")]
    #[case::scrolled_from_start(b"-AAAAAA-", 1..6, "  . ›")]
    #[case::short_of_end(b"-AAAAAA-", 0..5, "‹  . ")]
    #[case::nothing_hidden(b"AAAAAA", 0..6, ".   . ")]
    fn ruler_marks_filtered_edges_only_at_window_edges(
        #[case] sequence: &[u8],
        #[case] columns: Range<usize>,
        #[case] expected: &str,
    ) {
        let mut session = session(&[sequence, sequence]);
        session
            .update(Position::default(), |state| state.set_gap_filter(Some(0.0)))
            .unwrap();
        let window = Window {
            columns,
            ..Window::default()
        };
        let area = Rect::new(0, 0, u16::try_from(window.columns.len()).unwrap(), 2);
        let mut buf = Buffer::empty(area);

        Ruler {
            session: &session,
            window: &window,
            theme: &ThemeState::default(),
        }
        .render(area, &mut buf);

        assert_eq!(buf_line(&buf, area, 1), expected);
    }

    fn buf_line(buf: &Buffer, area: Rect, y: u16) -> String {
        let row = Rect {
            y,
            height: 1,
            ..area
        };
        let text = buffer_text(buf, row);
        format!("{text:<width$}", width = usize::from(area.width))
    }
}
