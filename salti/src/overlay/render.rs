use crate::core::model::AlignmentModel;
use crate::ui::notification::render_notification;
use crate::ui::ui_state::UiState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Block;

use super::minimap;
use super::overlay_state::ActiveOverlay;

pub fn render_overlays(
    f: &mut Frame,
    content_area: Rect,
    input_area: Rect,
    alignment: Option<&AlignmentModel>,
    ui: &UiState,
) {
    match &ui.overlay.active_overlay {
        Some(ActiveOverlay::Minimap(_)) => {
            if let Some(alignment) = alignment {
                minimap::render(f, content_area, input_area, alignment, ui);
            }
        }
        Some(ActiveOverlay::Palette(palette)) => {
            palette.render(f, content_area, input_area, &ui.theme.styles);
        }
        None => (),
    }

    if ui.overlay.active_overlay.is_none() {
        match ui.notification.as_ref() {
            Some(notification) => {
                render_notification(f, input_area, notification, &ui.theme.styles);
            }
            None => {
                f.render_widget(Block::new().style(ui.theme.styles.base_block), input_area);
            }
        }
    }
}
