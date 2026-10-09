use std::ops::Range;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Span,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget},
};

use crate::{
    config::theme::SequenceStyles,
    core::columns::{Cell, WindowColumns},
};

/// Lookup table that maps each byte value (`0-255`) to a str for display.
///
/// All printable ASCII bytes map to themselves.
/// Any byte not mapped to one of those outputs is rendered as `"?"`.
// TODO: This was originally for quick rendering of IUPAC bases and handling any chars outside of those.
// Revisit this now we have full ASCII mapping.
const BYTE_TO_CHAR: [&str; 256] = [
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", " ", "!", "\"", "#", "$", "%",
    "&", "'", "(", ")", "*", "+", ",", "-", ".", "/", "0", "1", "2", "3", "4", "5", "6", "7", "8",
    "9", ":", ";", "<", "=", ">", "?", "@", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K",
    "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "[", "\\", "]", "^",
    "_", "`", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
    "r", "s", "t", "u", "v", "w", "x", "y", "z", "{", "|", "}", "~", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?",
];

pub fn stretch(
    columns: &WindowColumns<'_>,
    bytes: &[u8],
    diff: Option<&[u8]>,
    styles: &SequenceStyles,
) -> Vec<Span<'static>> {
    let letters = styles.for_type(columns.grid.alignment_type());
    columns
        .cells
        .iter()
        .map(|cell| {
            let Some(Cell { index, centre }) = *cell else {
                return Span::raw(" ");
            };
            let byte = bytes[index];
            let matches = diff.is_some_and(|diff| diff[index] == byte);
            let style = letters[usize::from(byte)];
            match (matches, centre) {
                // diff mode rendering
                (true, true) => Span::styled(".", styles.diff_match),
                (true, false) => Span::raw(" "),
                (false, true) => Span::styled(BYTE_TO_CHAR[usize::from(byte)], style),
                // translation overlay rendering
                (false, false) => Span::styled(" ", style),
            }
        })
        .collect()
}

pub fn render_column_scrollbar(
    thumb: &str,
    thumb_style: Style,
    total: usize,
    window: &Range<usize>,
    area: Rect,
    buf: &mut Buffer,
) {
    if total <= window.len() {
        return;
    }

    Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(None)
        .thumb_symbol(thumb)
        .thumb_style(thumb_style)
        .render(
            area,
            buf,
            &mut ScrollbarState::new(total - window.len() + 1)
                .position(window.start)
                .viewport_content_length(window.len()),
        );
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{test_utils::session, ui::ui_state::ThemeState};

    fn stretched(cell: Option<Cell>, byte: u8, diff: Option<u8>) -> String {
        let session = session(&[b"A"]);
        let columns = WindowColumns {
            grid: session.grid(),
            columns: vec![0],
            cells: vec![cell],
            summaries: Vec::new(),
        };
        let diff = diff.map(|byte| vec![byte]);

        stretch(
            &columns,
            &[byte],
            diff.as_deref(),
            &ThemeState::default().sequence,
        )
        .into_iter()
        .map(|span| span.content.into_owned())
        .collect()
    }

    const CENTRE: Option<Cell> = Some(Cell {
        index: 0,
        centre: true,
    });
    const FLANK: Option<Cell> = Some(Cell {
        index: 0,
        centre: false,
    });

    #[rstest]
    #[case::centre_shows_letter(CENTRE, b'A', None, "A")]
    #[case::flank_is_blank(FLANK, b'A', None, " ")]
    #[case::no_cell_is_blank(None, b'A', None, " ")]
    #[case::diff_match_centre_is_dot(CENTRE, b'A', Some(b'A'), ".")]
    #[case::diff_match_flank_is_blank(FLANK, b'A', Some(b'A'), " ")]
    #[case::diff_mismatch_shows_letter(CENTRE, b'G', Some(b'A'), "G")]
    #[case::unprintable_is_question_mark(CENTRE, 0x07, None, "?")]
    fn stretch_works(
        #[case] cell: Option<Cell>,
        #[case] byte: u8,
        #[case] diff: Option<u8>,
        #[case] expected: &str,
    ) {
        assert_eq!(stretched(cell, byte, diff), expected);
    }

    #[test]
    fn stretch_reads_byte_at_cell_index() {
        let session = session(&[b"AC"]);
        let columns = WindowColumns {
            grid: session.grid(),
            columns: vec![0, 1],
            cells: vec![
                Some(Cell {
                    index: 1,
                    centre: true,
                }),
                Some(Cell {
                    index: 0,
                    centre: true,
                }),
            ],
            summaries: Vec::new(),
        };

        let text: String = stretch(
            &columns,
            b"AC",
            Some(b"AG"),
            &ThemeState::default().sequence,
        )
        .into_iter()
        .map(|span| span.content.into_owned())
        .collect();

        assert_eq!(text, "C.");
    }
}
