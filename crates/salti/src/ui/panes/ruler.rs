use std::ops::RangeInclusive;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Styled,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::{config::theme::Theme, session::Session, ui::layout::Window};

// breaks this close together are drawn as one `~` span
const DENSE_BREAK_DISTANCE: usize = 3;

pub struct Ruler<'frame> {
    pub session: &'frame Session,
    pub window: &'frame Window,
    pub theme: &'frame Theme,
}

impl Widget for Ruler<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let columns = self.session.layout().columns();
        let (Some(&first), Some(&last)) = (columns.first(), columns.last()) else {
            return;
        };
        let shown = self.window.columns.clone();
        let filtered_leading = shown.start == 0 && first > 0;
        let filtered_trailing =
            shown.end == columns.len() && last < self.session.grid().width() - 1;

        let (number_line, marker_line) = build_ruler(
            &columns[shown],
            filtered_leading,
            filtered_trailing,
            self.theme,
        );
        Paragraph::new(vec![number_line, marker_line])
            .style(self.theme.base)
            .render(area, buf);
    }
}

// where hidden columns sit next to a shown one: before it for Leading, after it for Trailing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakMarker {
    Leading,
    Trailing,
}

fn build_ruler(
    absolute_columns: &[usize],
    filtered_leading: bool,
    filtered_trailing: bool,
    theme: &Theme,
) -> (Line<'static>, Line<'static>) {
    let width = absolute_columns.len();
    if width == 0 {
        return (Line::from(""), Line::from(""));
    }

    let mut number_line = vec![Span::raw(" "); width];
    let mut marker_line = vec![Span::raw(" "); width];
    let breaks = break_positions(absolute_columns, filtered_leading, filtered_trailing);
    let fragmented_view = !breaks.is_empty();

    for (index, marker_span) in marker_line.iter_mut().enumerate() {
        let column_number = absolute_columns[index] + 1;
        if column_number != 1 && !column_number.is_multiple_of(5) {
            continue;
        }
        let is_major_tick = column_number.is_multiple_of(10);
        *marker_span = if is_major_tick {
            "|".set_style(theme.accent)
        } else {
            ".".set_style(theme.text_dim)
        };

        // after a break, the first tick gets a number so the jump is easy to read
        let is_run_start = fragmented_view
            && (index == 0 || absolute_columns[index - 1] + 1 != absolute_columns[index]);
        if is_major_tick || column_number == 1 || is_run_start {
            add_number(&mut number_line, index, column_number, theme);
        }
    }

    let dense_spans = dense_break_spans(&breaks, width);

    for (position, marker) in breaks {
        let marker_position = dense_break_marker_position(position, marker, width);
        if dense_spans
            .iter()
            .any(|span| span.contains(&marker_position))
        {
            continue;
        }

        let symbol = match marker {
            BreakMarker::Leading => "‹",
            BreakMarker::Trailing => "›",
        };
        marker_line[position] = symbol.set_style(theme.warning);
    }

    for span in dense_spans {
        for marker in &mut marker_line[span] {
            *marker = "~".set_style(theme.warning);
        }
    }

    (Line::from(number_line), Line::from(marker_line))
}

fn break_positions(
    absolute_columns: &[usize],
    filtered_leading: bool,
    filtered_trailing: bool,
) -> Vec<(usize, BreakMarker)> {
    let width = absolute_columns.len();
    debug_assert!(width > 0, "the ruler has no columns to mark breaks in");
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

fn add_number(number_line: &mut [Span<'static>], centre: usize, number: usize, theme: &Theme) {
    let number_string = number.to_string();
    let number_length = number_string.len();
    let ruler_width = number_line.len();
    let start_idx = centre
        .saturating_sub(number_length / 2)
        .min(ruler_width.saturating_sub(number_length));
    let left_padding = start_idx.saturating_sub(1);
    let right_padding = (start_idx + number_length + 1).min(ruler_width);

    // left out if it would overlap existing number
    if number_line[left_padding..right_padding]
        .iter()
        .any(|span| span.content.as_ref() != " ")
    {
        return;
    }

    for (offset, digit) in number_string.chars().enumerate() {
        if let Some(cell) = number_line.get_mut(start_idx + offset) {
            *cell = digit.to_string().set_style(theme.accent);
        }
    }
}

fn dense_break_spans(breaks: &[(usize, BreakMarker)], width: usize) -> Vec<RangeInclusive<usize>> {
    let marker_positions: Vec<usize> = breaks
        .iter()
        .map(|&(position, marker)| dense_break_marker_position(position, marker, width))
        .collect();
    let mut spans = Vec::new();
    let mut cluster_start = 0;

    while cluster_start < marker_positions.len() {
        let mut cluster_end = cluster_start + 1;
        while cluster_end < marker_positions.len()
            && marker_positions[cluster_end]
                <= marker_positions[cluster_end - 1] + DENSE_BREAK_DISTANCE
        {
            cluster_end += 1;
        }

        if cluster_end - cluster_start >= 2 {
            spans.push(marker_positions[cluster_start]..=marker_positions[cluster_end - 1]);
        }

        cluster_start = cluster_end;
    }

    spans
}

fn dense_break_marker_position(position: usize, marker: BreakMarker, width: usize) -> usize {
    match marker {
        BreakMarker::Leading => position,
        // counts from the column after the break, unless it's the last column
        BreakMarker::Trailing => (position + 1).min(width - 1),
    }
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use std::ops::Range;

    use BreakMarker::{Leading, Trailing};
    use rstest::rstest;

    use super::*;
    use crate::{
        session::Position,
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
            &Theme::default(),
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

    #[test]
    fn break_positions_is_empty_when_contiguous() {
        assert_eq!(break_positions(&[0, 1, 2], false, false), []);
    }

    #[rstest]
    #[case::within_three(&[(0, Trailing), (3, Trailing)], &[1..=4])]
    #[case::leading_marks_own_column(&[(0, Leading), (1, Trailing)], &[0..=2])]
    #[case::trailing_at_end_stays(&[(8, Trailing), (9, Trailing)], &[9..=9])]
    #[case::two_clusters(&[(0, Trailing), (1, Trailing), (6, Trailing), (7, Trailing)], &[1..=2, 7..=8])]
    fn dense_break_spans_works(
        #[case] breaks: &[(usize, BreakMarker)],
        #[case] expected: &[RangeInclusive<usize>],
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
            theme: &Theme::default(),
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
