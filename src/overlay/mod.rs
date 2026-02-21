mod command_palette;
mod minimap;

use crate::core::CoreState;
use crate::ui::UiState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

pub use command_palette::CommandPaletteState;
pub use minimap::MinimapState;

#[derive(Debug, Default)]
pub struct OverlayState {
    pub palette: Option<CommandPaletteState>,
    pub minimap: Option<MinimapState>,
    pub command_error: Option<String>,
}

pub fn render_overlays(
    f: &mut Frame,
    content_area: Rect,
    input_area: Rect,
    core: &CoreState,
    ui: &UiState,
) {
    if let Some(palette) = ui.overlay.palette.as_ref() {
        palette.render(f, content_area, input_area, &ui.theme_styles);
    } else if ui.overlay.minimap.is_some() {
        minimap::render(
            f,
            content_area,
            input_area,
            core,
            &ui.theme,
            &ui.theme_styles,
        );
    } else if let Some(message) = ui.overlay.command_error.as_deref() {
        let line = Line::from(vec![
            Span::styled("Error: ", ui.theme_styles.error),
            Span::styled(message, ui.theme_styles.error),
        ]);
        f.render_widget(
            Paragraph::new(line).style(ui.theme_styles.base_block),
            input_area,
        );
    } else {
        f.render_widget(Block::new().style(ui.theme_styles.base_block), input_area);
    }
}
