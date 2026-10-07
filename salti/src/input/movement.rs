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
