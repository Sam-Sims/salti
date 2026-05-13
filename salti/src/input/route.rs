use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::ui::{
    layers::state::ActiveLayer,
    layout::{AppLayout, FrameLayout},
    ui_state::UiState,
};

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
    use crate::{
        cli::StartupState,
        ui::{layers::palette::CommandPaletteState, layout::AlignmentHeaderLayout},
    };

    fn ui_state() -> UiState {
        UiState::new(StartupState {
            file_path: None,
            initial_position: 0,
        })
    }

    fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[test]
    fn key_uses_palette_when_open() {
        let mut ui = ui_state();
        ui.layers.open_palette(CommandPaletteState::empty());

        let route = route_key(&ui);

        assert_eq!(route, KeyRoute::Palette);
    }

    #[test]
    fn key_uses_global_without_palette() {
        let ui = ui_state();

        let route = route_key(&ui);

        assert_eq!(route, KeyRoute::Global);
    }

    #[test]
    fn palette_captures_mouse() {
        let mut ui = ui_state();
        ui.layers.open_palette(CommandPaletteState::empty());
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            5,
            AlignmentHeaderLayout::without_features(),
        );
        let mouse = mouse_event(
            MouseEventKind::Moved,
            app_layout.gff_pane_rows.x,
            app_layout.gff_pane_rows.y,
        );

        let route = route_mouse(&ui, &frame_layout, &app_layout, mouse, true);

        assert_eq!(route, MouseRoute::Palette);
    }

    #[test]
    fn minimap_captures_left_mouse_inside_track() {
        let mut ui = ui_state();
        ui.layers.toggle_minimap();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );
        let mouse = mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            frame_layout.overlay_area.x + frame_layout.overlay_area.width - 2,
            frame_layout.overlay_area.y + frame_layout.overlay_area.height - 2,
        );

        let route = route_mouse(&ui, &frame_layout, &app_layout, mouse, false);

        assert_eq!(route, MouseRoute::Minimap);
    }

    #[test]
    fn gff_pane_captures_hover() {
        let ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            5,
            AlignmentHeaderLayout::without_features(),
        );
        let mouse = mouse_event(
            MouseEventKind::Moved,
            app_layout.gff_pane_rows.x,
            app_layout.gff_pane_rows.y,
        );

        let route = route_mouse(&ui, &frame_layout, &app_layout, mouse, true);

        assert_eq!(route, MouseRoute::GffPane);
    }

    #[test]
    fn gff_pane_ignored_without_gff() {
        let ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            5,
            AlignmentHeaderLayout::without_features(),
        );
        let mouse = mouse_event(
            MouseEventKind::Moved,
            app_layout.gff_pane_rows.x,
            app_layout.gff_pane_rows.y,
        );

        let route = route_mouse(&ui, &frame_layout, &app_layout, mouse, false);

        assert_eq!(route, MouseRoute::Alignment);
    }

    #[test]
    fn mouse_defaults_to_alignment() {
        let ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );
        let mouse = mouse_event(MouseEventKind::Moved, 0, 0);

        let route = route_mouse(&ui, &frame_layout, &app_layout, mouse, false);

        assert_eq!(route, MouseRoute::Alignment);
    }

    #[test]
    fn minimap_falls_through_to_gff_pane_outside_track() {
        let mut ui = ui_state();
        ui.layers.toggle_minimap();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            5,
            AlignmentHeaderLayout::without_features(),
        );
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
