use ratatui::{buffer::CellWidth, symbols::merge::MergeStrategy, widgets::Block};

use crate::config::theme::ThemeStyles;

pub fn pane_block(styles: &ThemeStyles) -> Block<'static> {
    Block::bordered()
        .border_style(styles.border)
        .style(styles.base_block)
        .merge_borders(MergeStrategy::Exact)
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
