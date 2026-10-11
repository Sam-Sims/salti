use std::ops::Range;

use ratatui::{
    buffer::{Buffer, CellWidth},
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use super::commands::PaletteCommand;
use crate::{
    config::theme::ThemeStyles,
    ui::{popups::COLUMN_GAP, truncate_label},
};

const MAX_LIST_ROWS: usize = 10;

pub(super) fn rows_above_prompt(
    items: &[String],
    command: Option<&PaletteCommand>,
    width: u16,
) -> u16 {
    let (_, list_rows) = grid(items, width);
    let doc_rows = command.map_or(0, |command| {
        let (doc, aliases) = doc_text(command, width);
        doc.len() + aliases.len() + 2
    });
    u16::try_from(list_rows + doc_rows).unwrap_or(u16::MAX)
}

pub(super) fn list_rows(items: &[String], width: u16) -> u16 {
    u16::try_from(grid(items, width).1).unwrap_or(u16::MAX)
}

pub(super) fn draw_list(
    area: Rect,
    buf: &mut Buffer,
    items: &[String],
    selected: Option<usize>,
    styles: &ThemeStyles,
) {
    let (columns, _) = grid(items, area.width);
    let rows = usize::from(area.height);
    if rows == 0 {
        return;
    }
    let column_width = usize::from(area.width) / columns;
    let label_width = column_width.saturating_sub(COLUMN_GAP);
    let page = visible_page(items.len(), selected, rows * columns);

    let lines: Vec<Line> = (0..rows)
        .map(|row| {
            let spans = (0..columns).filter_map(|column| {
                let index = page.start + column * rows + row;
                if !page.contains(&index) {
                    return None;
                }
                let style = if selected == Some(index) {
                    styles.selection
                } else {
                    styles.text
                };
                let (label, padding) = pad_label(&items[index], label_width);
                Some([
                    Span::styled(label, style),
                    Span::raw(padding + &" ".repeat(COLUMN_GAP)),
                ])
            });
            Line::from(spans.flatten().collect::<Vec<_>>())
        })
        .collect();
    Paragraph::new(lines).render(area, buf);
}

pub(super) fn draw_doc(
    area: Rect,
    buf: &mut Buffer,
    command: &PaletteCommand,
    styles: &ThemeStyles,
) {
    if area.height < 3 {
        return;
    }
    let block = Block::bordered()
        .border_style(styles.border)
        .style(styles.panel_block_dim);
    let inner = block.inner(area);
    block.render(area, buf);
    let (doc, aliases) = doc_text(command, area.width);
    let lines: Vec<Line> = doc
        .into_iter()
        .map(|line| Line::styled(line, styles.text))
        .chain(
            aliases
                .into_iter()
                .map(|line| Line::styled(line, styles.text_muted)),
        )
        .collect();
    Paragraph::new(lines).render(inner, buf);
}

fn grid(items: &[String], width: u16) -> (usize, usize) {
    if items.is_empty() {
        return (1, 0);
    }
    let longest = items
        .iter()
        .map(|item| usize::from(item.as_str().cell_width()))
        .max()
        .unwrap_or_default();
    let columns = (usize::from(width) / (longest + COLUMN_GAP)).max(1);
    let rows = items.len().div_ceil(columns).min(MAX_LIST_ROWS);
    (columns, rows)
}

fn visible_page(count: usize, selected: Option<usize>, page_size: usize) -> Range<usize> {
    let start = selected.unwrap_or_default() / page_size * page_size;
    start..count.min(start + page_size)
}

fn doc_text(command: &PaletteCommand, width: u16) -> (Vec<String>, Vec<String>) {
    let width = usize::from(width.saturating_sub(2));
    let aliases = if command.aliases.is_empty() {
        Vec::new()
    } else {
        wrap_text(&format!("Aliases: {}", command.aliases.join(", ")), width)
    };
    (wrap_text(command.doc, width), aliases)
}

fn pad_label(label: &str, width: usize) -> (String, String) {
    let text = truncate_label(label, width);
    let padding = width.saturating_sub(usize::from(text.cell_width()));
    (text, " ".repeat(padding))
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let current = lines.last_mut().expect("lines starts with one line");
        let fits = usize::from(current.as_str().cell_width()) + 1 + usize::from(word.cell_width())
            <= width;
        if current.is_empty() || fits {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        } else {
            lines.push(word.to_string());
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn items(labels: &[&str]) -> Vec<String> {
        labels.iter().map(ToString::to_string).collect()
    }

    #[rstest]
    #[case::empty(&[], 20, (1, 0))]
    #[case::one_row(&["ab", "cd", "ef"], 20, (5, 1))]
    #[case::column_width_from_longest(&["abcdef", "ab", "cd"], 16, (2, 2))]
    #[case::wider_than_area(&["abcdefghij"], 5, (1, 1))]
    #[case::rows_capped(&["a"; 30], 3, (1, 10))]
    fn grid_works(#[case] labels: &[&str], #[case] width: u16, #[case] expected: (usize, usize)) {
        assert_eq!(grid(&items(labels), width), expected);
    }

    #[rstest]
    #[case::first_page_without_selection(25, None, 10, 0..10)]
    #[case::page_holding_selection(25, Some(14), 10, 10..20)]
    #[case::last_page_partial(25, Some(24), 10, 20..25)]
    fn visible_page_works(
        #[case] count: usize,
        #[case] selected: Option<usize>,
        #[case] page_size: usize,
        #[case] expected: Range<usize>,
    ) {
        assert_eq!(visible_page(count, selected, page_size), expected);
    }
}
