use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::config::keybindings::{KeyBindingCategory, KeyBindings, format_key_for_display};

const HELP_WIDTH: u16 = 60;
const HELP_HEIGHT: u16 = 20;

#[derive(Debug, Default)]
pub struct HelpComponent;

impl HelpComponent {
    fn centered_rect(area: Rect) -> Rect {
        let popup_width = HELP_WIDTH.min(area.width.saturating_sub(4));
        let popup_height = HELP_HEIGHT.min(area.height.saturating_sub(4));

        let x = area.x + (area.width - popup_width) / 2;
        let y = area.y + (area.height - popup_height) / 2;

        Rect {
            x,
            y,
            width: popup_width,
            height: popup_height,
        }
    }

    fn format_binding_line(binding: &crate::config::keybindings::KeyBinding) -> Line {
        let key_text = match &binding.alt_key {
            Some(alt) => format!(
                "{}, {}",
                format_key_for_display(binding.key, binding.modifiers),
                format_key_for_display(*alt, binding.modifiers)
            ),
            None => format_key_for_display(binding.key, binding.modifiers),
        };

        Line::from(vec![
            Span::styled(
                format!("  {key_text:<8}"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" - {}", binding.description)),
        ])
    }

    fn category_header(title: &str) -> Line {
        Line::from(vec![Span::styled(
            format!("  {title}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )])
    }

    fn generate_help_content(keybindings: &KeyBindings) -> Vec<Line> {
        let mut content = vec![Line::from("")];

        let (app_bindings, nav_bindings): (Vec<_>, Vec<_>) = keybindings
            .bindings
            .iter()
            .partition(|b| b.category == KeyBindingCategory::Application);

        if !app_bindings.is_empty() {
            content.push(HelpComponent::category_header("Application"));
            content.push(Line::from(""));

            for binding in &app_bindings {
                content.push(HelpComponent::format_binding_line(binding));
            }

            content.push(Line::from(""));
        }

        if !nav_bindings.is_empty() {
            content.push(HelpComponent::category_header("Navigation"));
            content.push(Line::from(""));

            for binding in &nav_bindings {
                content.push(HelpComponent::format_binding_line(binding));
            }

            content.push(Line::from(""));
        }

        content.push(Line::from(vec![Span::styled(
            "  Press any key to close help  ",
            Style::default().fg(Color::Gray),
        )]));

        content
    }

    pub fn render(&mut self, f: &mut Frame, full_area: Rect, keybindings: &KeyBindings) {
        let popup_area = HelpComponent::centered_rect(full_area);

        Clear.render(popup_area, f.buffer_mut());

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Line::from(vec![Span::styled(
                " Keyboard Shortcuts ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]))
            .style(Style::default().bg(Color::Black));

        let help_content = HelpComponent::generate_help_content(keybindings);
        let inner_area = block.inner(popup_area);

        f.render_widget(block, popup_area);
        f.render_widget(
            Paragraph::new(help_content)
                .alignment(Alignment::Left)
                .style(Style::default().bg(Color::Black)),
            inner_area,
        );
    }
}
