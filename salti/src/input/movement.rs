use std::ops::Range;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::command::Command;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct HorizontalDrag {
    anchor: Option<usize>,
}

impl HorizontalDrag {
    pub(crate) fn is_dragging(&self) -> bool {
        self.anchor.is_some()
    }

    pub(crate) fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        window_columns: &Range<usize>,
        total_columns: usize,
    ) -> Option<Command> {
        let window_span = window_columns.len();
        let in_area = area.contains((mouse.column, mouse.row).into());

        let (anchor, column) = match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) if in_area => {
                let column = column_at(mouse.column, area, total_columns);
                let anchor = if window_columns.contains(&column) {
                    column - window_columns.start
                } else {
                    window_span / 2
                };
                self.anchor = Some(anchor);
                (anchor, column)
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let anchor = self.anchor?;
                let column = column_at(mouse.column, area, total_columns);
                (anchor, column)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let anchor = self.anchor.take()?;
                if !in_area {
                    return None;
                }
                let column = column_at(mouse.column, area, total_columns);
                (anchor, column)
            }
            _ => return None,
        };

        let visible_target = column.saturating_sub(anchor);
        Some(Command::JumpToIndex(visible_target))
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
    #[case::click_in_window_keeps_place(&[(DOWN, 12)], &[15])]
    #[case::click_outside_window_centres(&[(DOWN, 16)], &[55])]
    #[case::drag_keeps_anchor(&[(DOWN, 12), (DRAG, 14)], &[15, 35])]
    #[case::release_in_area_jumps(&[(DOWN, 12), (UP, 14)], &[15, 35])]
    #[case::release_outside_area_ignored(&[(DOWN, 12), (UP, 30)], &[15])]
    #[case::release_ends_drag(&[(DOWN, 12), (UP, 12), (DRAG, 14)], &[15, 15])]
    #[case::drag_without_press(&[(DRAG, 14)], &[])]
    #[case::press_outside_area(&[(DOWN, 30), (DRAG, 14)], &[])]
    #[case::right_click(&[(MouseEventKind::Down(MouseButton::Right), 12)], &[])]
    fn handle_mouse_works(#[case] events: &[(MouseEventKind, u16)], #[case] expected: &[usize]) {
        let mut drag = HorizontalDrag::default();

        let jumps: Vec<Command> = events
            .iter()
            .filter_map(|&(kind, x)| {
                drag.handle_mouse(mouse_event(kind, x, 0), AREA, &(15..25), 100)
            })
            .collect();

        assert_eq!(
            jumps,
            expected
                .iter()
                .map(|&index| Command::JumpToIndex(index))
                .collect::<Vec<_>>()
        );
    }
}
