use crate::core::model::AlignmentModel;
use crate::ui::layers::minimap::Minimap;
use crate::ui::layers::notification::render_notification;
use crate::ui::layers::state::ActiveLayer;
use crate::ui::ui_state::UiState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Block;

pub fn render_overlays(
    f: &mut Frame,
    content_area: Rect,
    input_area: Rect,
    alignment: Option<&AlignmentModel>,
    ui: &UiState,
) {
    match &ui.layers.active {
        Some(ActiveLayer::Minimap(_)) => {
            if let Some(alignment) = alignment {
                f.render_widget(Minimap::new(input_area, alignment, ui), content_area);
            }
        }
        Some(ActiveLayer::Palette(palette)) => {
            palette.render(f, content_area, input_area, &ui.theme.styles);
        }
        None => (),
    }

    if ui.layers.active.is_none() {
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
