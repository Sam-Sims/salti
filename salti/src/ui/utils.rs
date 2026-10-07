use ratatui::{
    buffer::{Buffer, CellWidth},
    layout::Rect,
    style::Styled,
    symbols::merge::MergeStrategy,
    text::Line,
    widgets::{Block, Widget},
};

use crate::config::theme::ThemeStyles;

pub fn render_pane(
    styles: &ThemeStyles,
    title: Option<Line<'_>>,
    area: Rect,
    buf: &mut Buffer,
) -> Rect {
    let mut block = Block::bordered()
        .border_style(styles.border)
        .style(styles.base_block)
        .merge_borders(MergeStrategy::Exact);
    if let Some(title) = title {
        block = block.title(title);
    }
    let inner_area = block.inner(area);
    block.render(area, buf);
    inner_area
}

pub fn separator_line(width: u16, styles: &ThemeStyles) -> Line<'static> {
    Line::from("─".repeat(usize::from(width)).set_style(styles.border))
}

pub fn truncate_label(value: &str, width: usize) -> String {
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
