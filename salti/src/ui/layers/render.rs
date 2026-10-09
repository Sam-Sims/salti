use ratatui::{Frame, widgets::Block};

use crate::{
    core::session::Session,
    ui::{
        layers::{minimap::Minimap, notification::render_notification, state::ActiveLayer},
        layout::ScreenLayout,
        ui_state::UiState,
    },
};

pub fn render_overlays(
    f: &mut Frame,
    screen: &ScreenLayout,
    session: Option<&Session>,
    ui: &UiState,
) {
    let (content_area, input_area) = (screen.frame.overlay_area, screen.frame.input_area);
    match &ui.layers.active {
        Some(ActiveLayer::Minimap(_)) => {
            if let Some(session) = session {
                f.render_widget(Minimap::new(screen, session, ui), content_area);
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
