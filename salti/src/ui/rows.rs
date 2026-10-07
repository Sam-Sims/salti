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
                // quick translate mode rendering
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
