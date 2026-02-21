use crate::core::CoreState;
use crate::ui::UiState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Block;

use super::minimap;
use super::notification::render_notification;

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
    } else if let Some(notification) = ui.overlay.notification.as_ref() {
        render_notification(f, input_area, notification, &ui.theme_styles);
    } else {
        f.render_widget(Block::new().style(ui.theme_styles.base_block), input_area);
    }
}
