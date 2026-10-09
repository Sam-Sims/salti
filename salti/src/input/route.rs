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
    use crossterm::event::{MouseButton, MouseEventKind};
    use ratatui::layout::Rect;
    use rstest::rstest;

    use super::*;
    use crate::{
        test_utils::{mouse_event, ui_state},
        ui::{layers::palette::CommandPaletteState, layout::AlignmentHeaderLayout},
    };

    const LEFT_DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
    const LEFT_DRAG: MouseEventKind = MouseEventKind::Drag(MouseButton::Left);
    const MIDDLE_DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Middle);

    type At = fn(&FrameLayout, &AppLayout) -> (u16, u16);

    fn layouts() -> (FrameLayout, AppLayout) {
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout =
            AppLayout::new(frame_layout.content_area, 5, AlignmentHeaderLayout::new(0));
        (frame_layout, app_layout)
    }

    fn gff(_: &FrameLayout, app_layout: &AppLayout) -> (u16, u16) {
        (app_layout.gff_pane_rows.x, app_layout.gff_pane_rows.y)
    }

    fn minimap_track(frame_layout: &FrameLayout, _: &AppLayout) -> (u16, u16) {
        let area = frame_layout.overlay_area;
        (area.right() - 2, area.bottom() - 2)
    }

    fn alignment(_: &FrameLayout, app_layout: &AppLayout) -> (u16, u16) {
        let area = app_layout.alignment_pane_sequence_rows;
        (area.x, area.y)
    }

    fn no_layer(_: &mut UiState) {}

    fn palette(ui: &mut UiState) {
        ui.layers.open_palette(CommandPaletteState::empty());
    }

    fn minimap(ui: &mut UiState) {
        ui.layers.toggle_minimap();
    }

    #[rstest]
    #[case::palette_captures_everything(
        palette,
        true,
        MouseEventKind::Moved,
        gff,
        MouseRoute::Palette
    )]
    #[case::minimap_track(minimap, false, LEFT_DOWN, minimap_track, MouseRoute::Minimap)]
    #[case::minimap_track_ignores_middle(
        minimap,
        false,
        MIDDLE_DOWN,
        minimap_track,
        MouseRoute::Alignment
    )]
    #[case::minimap_falls_through_to_gff(minimap, true, LEFT_DOWN, gff, MouseRoute::GffPane)]
    #[case::minimap_ignores_drag_it_didnt_start(
        minimap,
        false,
        LEFT_DRAG,
        alignment,
        MouseRoute::Alignment
    )]
    #[case::gff_hover(no_layer, true, MouseEventKind::Moved, gff, MouseRoute::GffPane)]
    #[case::gff_left_click(no_layer, true, LEFT_DOWN, gff, MouseRoute::GffPane)]
    #[case::gff_ignores_middle(no_layer, true, MIDDLE_DOWN, gff, MouseRoute::Alignment)]
    #[case::gff_ignored_without_gff(
        no_layer,
        false,
        MouseEventKind::Moved,
        gff,
        MouseRoute::Alignment
    )]
    #[case::gff_ignores_drag_it_didnt_start(
        no_layer,
        true,
        LEFT_DRAG,
        alignment,
        MouseRoute::Alignment
    )]
    #[case::alignment(no_layer, true, LEFT_DOWN, alignment, MouseRoute::Alignment)]
    fn route_mouse_works(
        #[case] open: fn(&mut UiState),
        #[case] has_gff: bool,
        #[case] kind: MouseEventKind,
        #[case] at: At,
        #[case] expected: MouseRoute,
    ) {
        let mut ui = ui_state();
        open(&mut ui);
        let (frame_layout, app_layout) = layouts();
        let (x, y) = at(&frame_layout, &app_layout);

        let route = route_mouse(
            &ui,
            &frame_layout,
            &app_layout,
            mouse_event(kind, x, y),
            has_gff,
        );

        assert_eq!(route, expected);
    }

    #[test]
    fn route_mouse_keeps_gff_drag_outside_pane() {
        let mut ui = ui_state();
        let (frame_layout, app_layout) = layouts();
        let (x, y) = gff(&frame_layout, &app_layout);
        ui.gff_pane.handle_mouse(
            mouse_event(LEFT_DOWN, x, y),
            app_layout.gff_pane_rows,
            &(0..10),
            100,
        );
        let (x, y) = alignment(&frame_layout, &app_layout);
        let drag = mouse_event(LEFT_DRAG, x, y);

        let route = route_mouse(&ui, &frame_layout, &app_layout, drag, true);

        assert_eq!(route, MouseRoute::GffPane);
    }
}
