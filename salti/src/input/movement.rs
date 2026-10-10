use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::command::Command;

pub(crate) fn centre_on_pointer(
    mouse: MouseEvent,
    area: Rect,
    window_span: usize,
    total_columns: usize,
) -> Option<Command> {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
            let column = column_at(mouse.column, area, total_columns);
            Some(Command::JumpToIndex(column.saturating_sub(window_span / 2)))
        }
        _ => None,
    }
}

fn column_at(mouse_x: u16, area: Rect, total_columns: usize) -> usize {
    let offset = usize::from(mouse_x.saturating_sub(area.x));
    let column = offset.saturating_mul(total_columns) / usize::from(area.width);
    column.min(total_columns.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_utils::mouse_event;

    const DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
    const DRAG: MouseEventKind = MouseEventKind::Drag(MouseButton::Left);
    const UP: MouseEventKind = MouseEventKind::Up(MouseButton::Left);
    const AREA: Rect = Rect::new(10, 0, 10, 1);

    #[rstest]
    #[case::first_cell(10, 100, 0)]
    #[case::scaled(15, 100, 50)]
    #[case::last_cell(19, 100, 90)]
    #[case::clamped_to_last_column(19, 5, 4)]
    #[case::left_of_area(5, 100, 0)]
    fn column_at_works(#[case] x: u16, #[case] total_columns: usize, #[case] expected: usize) {
        assert_eq!(column_at(x, AREA, total_columns), expected);
    }

    #[rstest]
    #[case::press_centres(DOWN, 15, Some(45))]
    #[case::drag_centres(DRAG, 12, Some(15))]
    #[case::near_start_clamps_to_zero(DOWN, 10, Some(0))]
    #[case::release_ignored(UP, 15, None)]
    #[case::right_click_ignored(MouseEventKind::Down(MouseButton::Right), 15, None)]
    fn handle_mouse_works(
        #[case] kind: MouseEventKind,
        #[case] x: u16,
        #[case] expected: Option<usize>,
    ) {
        assert_eq!(
            centre_on_pointer(mouse_event(kind, x, 0), AREA, 10, 100),
            expected.map(Command::JumpToIndex)
        );
    }
}
