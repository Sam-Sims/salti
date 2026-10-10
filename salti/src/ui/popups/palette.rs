mod command_definitions;
mod command_runners;
mod command_spec;
mod completers;
mod input;
mod ui;

pub use input::CommandPaletteState;
use ratatui::buffer::CellWidth;

use crate::ui::truncate_label;

fn parse_argument(input: &str) -> Option<String> {
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for character in input.chars() {
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                current.push(character);
            }
            continue;
        }

        if character == '"' || character == '\'' {
            quote = Some(character);
        } else if character.is_whitespace() {
            if !current.is_empty() {
                break;
            }
        } else {
            current.push(character);
        }
    }

    (!current.is_empty()).then_some(current)
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

    let mut lines = Vec::new();
    for raw_line in text.split('\n') {
        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
                continue;
            }
            if current.len() + 1 + word.len() > width {
                lines.push(current);
                current = word.to_string();
            } else {
                current.push(' ');
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
