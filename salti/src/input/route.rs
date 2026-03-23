use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::overlay::overlay_state::ActiveOverlay;
use crate::ui::layout::FrameLayout;
use crate::ui::ui_state::UiState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyRoute {
    Palette,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MouseRoute {
    Palette,
    Minimap,
    Alignment,
}

pub(super) fn route_key(ui: &UiState) -> KeyRoute {
    match &ui.overlay.active_overlay {
        Some(ActiveOverlay::Palette(_)) => KeyRoute::Palette,
        _ => KeyRoute::Global,
    }
}

pub(super) fn route_mouse(
    ui: &UiState,
    frame_layout: &FrameLayout,
    mouse: MouseEvent,
) -> MouseRoute {
    match &ui.overlay.active_overlay {
        Some(ActiveOverlay::Palette(_)) => MouseRoute::Palette,
        Some(ActiveOverlay::Minimap(minimap_state)) => {
            let left_mouse = matches!(
                mouse.kind,
                MouseEventKind::Down(MouseButton::Left)
                    | MouseEventKind::Drag(MouseButton::Left)
                    | MouseEventKind::Up(MouseButton::Left)
            );

            if (left_mouse && minimap_state.contains_mouse(mouse, frame_layout.overlay_area))
                || (matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left))
                    && minimap_state.is_dragging())
            {
                MouseRoute::Minimap
            } else {
                MouseRoute::Alignment
            }
        }
        None => MouseRoute::Alignment,
    }
}
