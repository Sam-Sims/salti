use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::ui::layout::AppLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MouseRoute {
    GffPane,
    Alignment,
}

pub(super) fn route_mouse(app_layout: &AppLayout, mouse: MouseEvent, has_gff: bool) -> MouseRoute {
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

        if in_gff && (is_left_mouse || is_hover) {
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
        test_utils::mouse_event,
        ui::layout::{AlignmentHeaderLayout, FrameLayout},
    };

    const LEFT_DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
    const LEFT_DRAG: MouseEventKind = MouseEventKind::Drag(MouseButton::Left);
    const MIDDLE_DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Middle);

    type At = fn(&AppLayout) -> (u16, u16);

    fn app_layout() -> AppLayout {
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        AppLayout::new(frame_layout.content_area, 5, AlignmentHeaderLayout::new(0))
    }

    fn gff(app_layout: &AppLayout) -> (u16, u16) {
        (app_layout.gff_pane_rows.x, app_layout.gff_pane_rows.y)
    }

    fn alignment(app_layout: &AppLayout) -> (u16, u16) {
        let area = app_layout.alignment_pane_sequence_rows;
        (area.x, area.y)
    }

    #[rstest]
    #[case::gff_hover(true, MouseEventKind::Moved, gff, MouseRoute::GffPane)]
    #[case::gff_left_click(true, LEFT_DOWN, gff, MouseRoute::GffPane)]
    #[case::gff_ignores_middle(true, MIDDLE_DOWN, gff, MouseRoute::Alignment)]
    #[case::gff_ignored_without_gff(false, MouseEventKind::Moved, gff, MouseRoute::Alignment)]
    #[case::gff_ignores_drag_it_didnt_start(true, LEFT_DRAG, alignment, MouseRoute::Alignment)]
    #[case::alignment(true, LEFT_DOWN, alignment, MouseRoute::Alignment)]
    fn route_mouse_works(
        #[case] has_gff: bool,
        #[case] kind: MouseEventKind,
        #[case] at: At,
        #[case] expected: MouseRoute,
    ) {
        let app_layout = app_layout();
        let (x, y) = at(&app_layout);

        let route = route_mouse(&app_layout, mouse_event(kind, x, y), has_gff);

        assert_eq!(route, expected);
    }
}
