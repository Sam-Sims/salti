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
        viewport_col_range: &Range<usize>,
        total_columns: usize,
        position_from_mouse: impl Fn(u16, Rect, usize) -> usize,
    ) -> Option<Command> {
        let viewport_span = viewport_col_range.len();
        let in_area = area.contains((mouse.column, mouse.row).into());

        let (anchor, column) = match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) if in_area => {
                let column = position_from_mouse(mouse.column, area, total_columns);
                let anchor = if viewport_col_range.contains(&column) {
                    column - viewport_col_range.start
                } else {
                    viewport_span / 2
                };
                self.anchor = Some(anchor);
                (anchor, column)
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let anchor = self.anchor?;
                let column = position_from_mouse(mouse.column, area, total_columns);
                (anchor, column)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let anchor = self.anchor.take()?;
                if !in_area {
                    return None;
                }
                let column = position_from_mouse(mouse.column, area, total_columns);
                (anchor, column)
            }
            _ => return None,
        };

        let visible_target = column.saturating_sub(anchor);
        Some(Command::JumpToPosition(visible_target))
    }
}
