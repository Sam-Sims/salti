use crate::config::theme::ThemeStyles;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub level: NotificationLevel,
    pub message: String,
}

fn notification_prefix(level: NotificationLevel, theme: &ThemeStyles) -> (&'static str, Style) {
    match level {
        NotificationLevel::Error => ("Error: ", theme.error),
        NotificationLevel::Warning => ("Warning: ", theme.warning),
        NotificationLevel::Info => ("", theme.success),
    }
}

pub fn render_notification(
    f: &mut Frame,
    input_area: Rect,
    notification: &Notification,
    theme: &ThemeStyles,
) {
    let (prefix, style) = notification_prefix(notification.level, theme);
    let line = Line::from(vec![
        Span::styled(prefix, style),
        Span::styled(notification.message.as_str(), style),
    ]);
    f.render_widget(Paragraph::new(line).style(theme.base_block), input_area);
}
