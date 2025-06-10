use crate::state::State;
use crate::viewport::Viewport;
use ratatui::Frame;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Block;

#[derive(Debug, Default)]
pub struct UiComponent;

impl UiComponent {
    fn build_status_bar(app_state: &State, viewport: &Viewport) -> Vec<Span<'static>> {
        let alignments = &app_state.alignments;
        let loading_state = &app_state.loading_state;
        let file_path = &app_state.file_path;
        let sequence_length = app_state.sequence_length;

        let file_name = file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown");

        let loading_status = {
            let text = loading_state.to_string();
            let style = match loading_state {
                crate::state::LoadingState::Loading => Style::default().fg(Color::Yellow),
                crate::state::LoadingState::Loaded => Style::default().fg(Color::Green),
            };
            Span::styled(text, style)
        };

        let position_range = if sequence_length > 0 {
            let range = viewport.horizontal_range();
            format!("Positions: {}-{}", range.start + 1, range.end)
        } else {
            "Positions: 0-0".to_string()
        };

        vec![
            Span::styled(
                format!("{} alignments", alignments.len()),
                Style::default().fg(Color::Gray),
            ),
            Span::raw(" | "),
            Span::styled(
                format!("File: {file_name}"),
                Style::default().fg(Color::Gray),
            ),
            Span::raw(" | "),
            loading_status,
            Span::raw(" | "),
            Span::styled(position_range, Style::default().fg(Color::Gray)),
            Span::raw(" | "),
        ]
    }

    fn build_top_bar() -> Vec<Span<'static>> {
        vec![
            Span::styled("Press 'q' to quit", Style::default().fg(Color::Gray)),
            Span::raw(" | "),
            Span::styled(
                "Use <🡰🡲> and <🡱🡳> to navigate",
                Style::default().fg(Color::Gray),
            ),
        ]
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        full_area: ratatui::layout::Rect,
        app_state: &State,
        viewport: &Viewport,
    ) {
        let status_bar = Self::build_status_bar(app_state, viewport);
        let top_bar = Self::build_top_bar();

        let block = Block::bordered()
            .title(Line::from(top_bar))
            .title_bottom(Line::from(status_bar));

        f.render_widget(block, full_area);
    }
}
