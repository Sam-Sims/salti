use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::ui::layers::state::ActiveLayer;
use crate::ui::layout::{AppLayout, FrameLayout};
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
    GffPane,
    Alignment,
}

pub(super) fn route_key(ui: &UiState) -> KeyRoute {
    match &ui.layers.active {
        Some(ActiveLayer::Palette(_)) => KeyRoute::Palette,
        _ => KeyRoute::Global,
    }
}

pub(super) fn route_mouse(
    ui: &UiState,
    frame_layout: &FrameLayout,
    app_layout: &AppLayout,
    mouse: MouseEvent,
    has_gff: bool,
) -> MouseRoute {
    match &ui.layers.active {
        Some(ActiveLayer::Palette(_)) => return MouseRoute::Palette,
        Some(ActiveLayer::Minimap(minimap_state)) => {
            let left_mouse = matches!(
                mouse.kind,
                MouseEventKind::Down(MouseButton::Left)
                    | MouseEventKind::Drag(MouseButton::Left)
                    | MouseEventKind::Up(MouseButton::Left)
            );

            let is_minimap_drag = minimap_state.is_dragging()
                && matches!(
                    mouse.kind,
                    MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
                );

            if (left_mouse && minimap_state.contains_mouse(mouse, frame_layout.overlay_area))
                || is_minimap_drag
            {
                return MouseRoute::Minimap;
            }
        }
        None => (),
    }

    if has_gff && app_layout.gff_pane_rows.height > 0 {
        let in_gff = app_layout
            .gff_pane_rows
            .contains((mouse.column, mouse.row).into());
        let is_left_mouse = matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Left)
                | MouseEventKind::Drag(MouseButton::Left)
                | MouseEventKind::Up(MouseButton::Left)
        );
        let is_hover = matches!(mouse.kind, MouseEventKind::Moved);
        let is_drag = ui.gff_pane.is_dragging()
            && matches!(
                mouse.kind,
                MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
            );

        if (in_gff && (is_left_mouse || is_hover)) || is_drag {
            return MouseRoute::GffPane;
        }
    }

    MouseRoute::Alignment
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    use super::*;
    use crate::cli::StartupState;

    fn ui_state() -> UiState {
        UiState::new(StartupState {
            file_path: None,
            initial_position: 0,
        })
    }

    #[test]
    fn minimap_falls_through_to_gff_pane_outside_track() {
        let mut ui = ui_state();
        ui.layers.toggle_minimap();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(frame_layout.content_area, 5);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: app_layout.gff_pane_rows.x,
            row: app_layout.gff_pane_rows.y,
            modifiers: KeyModifiers::empty(),
        };

        let route = route_mouse(&ui, &frame_layout, &app_layout, mouse, true);

        assert_eq!(route, MouseRoute::GffPane);
    }
}
