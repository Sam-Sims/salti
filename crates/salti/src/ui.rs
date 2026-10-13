#![allow(clippy::cast_possible_truncation)]

pub(crate) mod bars;
pub(crate) mod layout;
pub(crate) mod notification;
pub(crate) mod panes;
pub(crate) mod popups;
pub(crate) mod render;
pub(crate) mod selection;
pub(crate) mod state;

use ratatui::{
    buffer::{Buffer, CellWidth},
    layout::{Constraint, HorizontalAlignment, Rect},
    style::Style,
    text::Line,
    widgets::{Paragraph, Widget},
};

pub(crate) fn input_name(input: &str) -> &str {
    std::path::Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(input)
}

fn render_centred_lines(lines: Vec<Line<'_>>, style: Style, area: Rect, buf: &mut Buffer) {
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    buf.set_style(area, style);
    Paragraph::new(lines)
        .alignment(HorizontalAlignment::Center)
        .style(style)
        .render(area.centered_vertically(Constraint::Length(height)), buf);
}

fn truncate_label(value: &str, width: usize) -> String {
    if usize::from(value.cell_width()) <= width {
        return value.to_string();
    }

    let budget = if width <= 3 { width } else { width - 3 };
    let mut used = 0;
    let mut text: String = value
        .chars()
        .take_while(|ch| {
            used += usize::from(ch.encode_utf8(&mut [0; 4]).cell_width());
            used <= budget
        })
        .collect();
    if width > 3 {
        text.push_str("...");
    }
    text
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::fits("abc", 5, "abc")]
    #[case::exact_fit("abcde", 5, "abcde")]
    #[case::ellipsis("abcdefgh", 6, "abc...")]
    #[case::too_narrow_for_ellipsis("abcdef", 3, "abc")]
    #[case::zero_width("abc", 0, "")]
    #[case::wide_characters("日本語テキスト", 7, "日本...")]
    #[case::wide_character_not_split("日本語", 5, "日...")]
    fn truncate_label_works(#[case] value: &str, #[case] width: usize, #[case] expected: &str) {
        assert_eq!(truncate_label(value, width), expected);
    }
}
