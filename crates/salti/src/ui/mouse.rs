use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::{
    command::Command,
    gff::Gff,
    session::Session,
    ui::{
        UiState,
        layout::{ScreenLayout, Window, screen_rows},
        panes::gff,
        selection::Selection,
    },
};

pub(crate) fn handle(
    tracker: &mut MouseTracker,
    session: Option<&Session>,
    gff: Option<&Gff>,
    ui: &mut UiState,
    screen: &ScreenLayout,
    mouse: MouseEvent,
) -> Vec<Command> {
    ui.gff_tooltip = None;
    let Some(session) = session else {
        return Vec::new();
    };
    if matches!(mouse.kind, MouseEventKind::Down(_)) {
        tracker.clear_anchors();
    }

    let point = (mouse.column, mouse.row).into();
    if let Some(gff) = gff
        && screen.app.gff_pane_rows.contains(point)
    {
        return handle_gff_mouse_event(gff, session, ui, screen, mouse);
    }
    if screen.app.alignment_pane_sequence_rows.contains(point) {
        return handle_alignment_mouse_event(tracker, session, ui, screen, mouse);
    }
    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
        ui.selection = None;
    }
    Vec::new()
}

#[derive(Debug, Default)]
pub(crate) struct MouseTracker {
    selection_drag: Option<SelectionDrag>,
    pan_anchor: Option<(u16, u16)>,
}

impl MouseTracker {
    pub fn clear_anchors(&mut self) {
        self.selection_drag = None;
        self.pan_anchor = None;
    }

    fn pan_drag_commands(&mut self, column: u16, row: u16) -> [Option<Command>; 2] {
        let Some((anchor_x, anchor_y)) = self.pan_anchor else {
            return [None, None];
        };

        let (row_delta, scroll_up) = if row >= anchor_y {
            (usize::from(row - anchor_y), true)
        } else {
            (usize::from(anchor_y - row), false)
        };

        let (column_delta, scroll_left) = if column >= anchor_x {
            (usize::from(column - anchor_x), true)
        } else {
            (usize::from(anchor_x - column), false)
        };

        self.pan_anchor = Some((column, row));

        [
            (row_delta > 0).then_some(if scroll_up {
                Command::ScrollUp { amount: row_delta }
            } else {
                Command::ScrollDown { amount: row_delta }
            }),
            (column_delta > 0).then_some(if scroll_left {
                Command::ScrollLeft {
                    amount: column_delta,
                }
            } else {
                Command::ScrollRight {
                    amount: column_delta,
                }
            }),
        ]
    }
}

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

#[derive(Debug, Clone, Copy)]
enum SelectionDrag {
    Follow,
    Box(Selection),
}

fn handle_gff_mouse_event(
    gff: &Gff,
    session: &Session,
    ui: &mut UiState,
    screen: &ScreenLayout,
    mouse: MouseEvent,
) -> Vec<Command> {
    let gff_pane_rows = screen.app.gff_pane_rows;
    let total_columns = session.layout().columns().len();

    ui.gff_tooltip = gff::tooltip_at(gff, session, gff_pane_rows, mouse.column, mouse.row);

    centre_on_pointer(
        mouse,
        gff::gff_content_area(gff_pane_rows, total_columns),
        screen.window.columns.len(),
        total_columns,
    )
    .into_iter()
    .collect()
}

fn handle_alignment_mouse_event(
    tracker: &mut MouseTracker,
    session: &Session,
    ui: &mut UiState,
    screen: &ScreenLayout,
    mouse: MouseEvent,
) -> Vec<Command> {
    let resolved_anchor = anchor_at(
        session,
        &screen.window,
        screen.app.alignment_pane_sequence_rows,
        mouse.column,
        mouse.row,
    );

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(anchor) = resolved_anchor else {
                ui.selection = None;
                return Vec::new();
            };
            tracker.selection_drag = Some(if mouse.modifiers.contains(KeyModifiers::CONTROL) {
                SelectionDrag::Box(anchor)
            } else {
                SelectionDrag::Follow
            });
            ui.selection = Some(anchor);
        }
        MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
            if let (Some(drag), Some(current)) = (tracker.selection_drag, resolved_anchor) {
                ui.selection = Some(match drag {
                    SelectionDrag::Follow => current,
                    SelectionDrag::Box(anchor) => selection_from_anchors(anchor, current),
                });
            }
            if mouse.kind == MouseEventKind::Up(MouseButton::Left) {
                tracker.selection_drag = None;
            }
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            tracker.pan_anchor = Some((mouse.column, mouse.row));
        }
        MouseEventKind::Drag(MouseButton::Middle) => {
            return tracker
                .pan_drag_commands(mouse.column, mouse.row)
                .into_iter()
                .flatten()
                .collect();
        }
        MouseEventKind::Up(MouseButton::Middle) => {
            tracker.pan_anchor = None;
        }
        _ => (),
    }
    Vec::new()
}

fn anchor_at(
    session: &Session,
    window: &Window,
    sequence_rows_area: Rect,
    mouse_x: u16,
    mouse_y: u16,
) -> Option<Selection> {
    debug_assert!(
        sequence_rows_area.contains((mouse_x, mouse_y).into()),
        "pointer should be inside the sequence rows"
    );

    let row = screen_rows(session.layout(), window)
        .nth(usize::from(mouse_y - sequence_rows_area.y))
        .flatten()?;
    let column = window
        .columns
        .clone()
        .nth(usize::from(mouse_x - sequence_rows_area.x))?;
    let columns = session.selection_columns((column..=column).into());

    Some(Selection {
        rows: (row..=row).into(),
        columns,
    })
}

fn selection_from_anchors(anchor: Selection, current: Selection) -> Selection {
    Selection {
        rows: (anchor.rows.start.min(current.rows.start)..=anchor.rows.last.max(current.rows.last))
            .into(),
        columns: (anchor.columns.start.min(current.columns.start)
            ..=anchor.columns.last.max(current.columns.last))
            .into(),
    }
}

fn column_at(mouse_x: u16, area: Rect, total_columns: usize) -> usize {
    let offset = usize::from(mouse_x.saturating_sub(area.x));
    let column = offset.saturating_mul(total_columns) / usize::from(area.width);
    column.min(total_columns.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use std::ops;

    use rstest::rstest;

    use super::*;
    use crate::{
        gff::{Feature, FeatureType, Strand},
        session::{Position, ViewState},
        test_utils::{mouse_event, pinned_session, screen, session, ui_state},
    };

    const LEFT_DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
    const LEFT_DRAG: MouseEventKind = MouseEventKind::Drag(MouseButton::Left);
    const LEFT_UP: MouseEventKind = MouseEventKind::Up(MouseButton::Left);
    const MIDDLE_DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Middle);
    const MIDDLE_DRAG: MouseEventKind = MouseEventKind::Drag(MouseButton::Middle);
    const MIDDLE_UP: MouseEventKind = MouseEventKind::Up(MouseButton::Middle);

    fn selection(
        rows: ops::RangeInclusive<usize>,
        columns: ops::RangeInclusive<usize>,
    ) -> Selection {
        Selection {
            rows: rows.into(),
            columns: columns.into(),
        }
    }

    const AREA: Rect = Rect::new(0, 0, 80, 24);

    fn pinned_screen() -> ScreenLayout {
        screen(
            AREA,
            0,
            Window {
                pinned: 0..1,
                rows: 0..4,
                columns: 0..20,
                ..Window::default()
            },
        )
    }

    fn gff_screen() -> ScreenLayout {
        screen(
            AREA,
            4,
            Window {
                rows: 0..1,
                columns: 0..60,
                ..Window::default()
            },
        )
    }

    fn handle_all(
        session: &Session,
        ui: &mut UiState,
        events: &[(MouseEventKind, KeyModifiers, u16, u16)],
    ) -> Vec<Command> {
        let screen = pinned_screen();
        let area = screen.app.alignment_pane_sequence_rows;
        let mut tracker = MouseTracker::default();
        events
            .iter()
            .flat_map(|&(kind, modifiers, x, y)| {
                let mouse = MouseEvent {
                    modifiers,
                    ..mouse_event(kind, area.x + x, area.y + y)
                };
                handle(&mut tracker, Some(session), None, ui, &screen, mouse)
            })
            .collect()
    }

    #[rstest]
    #[case::pinned_row(10, 5, Some(selection(0..=0, 5..=5)))]
    #[case::main_row(12, 7, Some(selection(1..=1, 7..=7)))]
    #[case::divider(10, 6, None)]
    #[case::past_window_rows(10, 9, None)]
    #[case::past_window_columns(18, 5, None)]
    fn anchor_at_works(#[case] x: u16, #[case] y: u16, #[case] expected: Option<Selection>) {
        let session = pinned_session(6, &[3]);
        let window = Window {
            pinned: 0..1,
            rows: 0..2,
            columns: 5..13,
            ..Window::default()
        };
        let area = Rect::new(10, 5, 10, 6);

        let anchor = anchor_at(&session, &window, area, x, y);

        assert_eq!(anchor, expected);
    }

    #[test]
    fn anchor_at_widens_to_codon_in_translation_overlay() {
        let mut session = session(&[b"ATGAAATTT"]);
        session
            .update(Position::default(), ViewState::toggle_translation_overlay)
            .unwrap();
        let window = Window {
            rows: 0..1,
            columns: 0..9,
            ..Window::default()
        };

        let anchor = anchor_at(&session, &window, Rect::new(0, 0, 9, 1), 4, 0);

        assert_eq!(anchor, Some(selection(0..=0, 3..=5)));
    }

    #[rstest]
    #[case::forward(selection(1..=1, 2..=2), selection(3..=3, 5..=5), selection(1..=3, 2..=5))]
    #[case::backward(selection(3..=3, 5..=5), selection(1..=1, 2..=2), selection(1..=3, 2..=5))]
    #[case::crossed(selection(1..=1, 5..=5), selection(3..=3, 2..=2), selection(1..=3, 2..=5))]
    #[case::codons(selection(0..=0, 3..=5), selection(0..=0, 0..=2), selection(0..=0, 0..=5))]
    fn selection_from_anchors_works(
        #[case] anchor: Selection,
        #[case] current: Selection,
        #[case] expected: Selection,
    ) {
        assert_eq!(selection_from_anchors(anchor, current), expected);
    }

    #[rstest]
    #[case::down_right(12, 13, &[Command::ScrollUp { amount: 3 }, Command::ScrollLeft { amount: 2 }])]
    #[case::up_left(8, 7, &[Command::ScrollDown { amount: 3 }, Command::ScrollRight { amount: 2 }])]
    #[case::vertical_only(10, 12, &[Command::ScrollUp { amount: 2 }])]
    #[case::no_movement(10, 10, &[])]
    fn pan_drag_commands_works(#[case] x: u16, #[case] y: u16, #[case] expected: &[Command]) {
        let mut tracker = MouseTracker {
            pan_anchor: Some((10, 10)),
            ..MouseTracker::default()
        };

        let commands: Vec<Command> = tracker
            .pan_drag_commands(x, y)
            .into_iter()
            .flatten()
            .collect();

        assert_eq!(commands, expected);
    }

    #[test]
    fn pan_drag_commands_measures_from_last_drag() {
        let mut tracker = MouseTracker {
            pan_anchor: Some((10, 10)),
            ..MouseTracker::default()
        };
        tracker.pan_drag_commands(10, 12);

        let commands = tracker.pan_drag_commands(10, 13);

        assert_eq!(commands, [Some(Command::ScrollUp { amount: 1 }), None]);
    }

    #[test]
    fn pan_drag_commands_rejects_without_anchor() {
        let mut tracker = MouseTracker::default();

        assert_eq!(tracker.pan_drag_commands(12, 12), [None, None]);
    }

    #[rstest]
    #[case::click(&[(LEFT_DOWN, KeyModifiers::NONE, 3, 2)], Some(selection(1..=1, 3..=3)))]
    #[case::drag_without_control_follows_mouse(
        &[(LEFT_DOWN, KeyModifiers::NONE, 0, 0), (LEFT_DRAG, KeyModifiers::NONE, 3, 3)],
        Some(selection(2..=2, 3..=3)),
    )]
    #[case::control_drag_from_pinned_into_main(
        &[(LEFT_DOWN, KeyModifiers::CONTROL, 0, 0), (LEFT_DRAG, KeyModifiers::NONE, 3, 3)],
        Some(selection(0..=2, 0..=3)),
    )]
    #[case::control_drag_within_main(
        &[(LEFT_DOWN, KeyModifiers::CONTROL, 1, 2), (LEFT_DRAG, KeyModifiers::NONE, 3, 4)],
        Some(selection(1..=3, 1..=3)),
    )]
    #[case::drag_over_divider_keeps_selection(
        &[(LEFT_DOWN, KeyModifiers::CONTROL, 0, 0), (LEFT_DRAG, KeyModifiers::NONE, 3, 1)],
        Some(selection(0..=0, 0..=0)),
    )]
    #[case::release_ends_box(
        &[(LEFT_DOWN, KeyModifiers::CONTROL, 0, 0), (LEFT_UP, KeyModifiers::NONE, 1, 0), (LEFT_DRAG, KeyModifiers::NONE, 3, 3)],
        Some(selection(0..=0, 0..=1)),
    )]
    fn handle_selects(
        #[case] events: &[(MouseEventKind, KeyModifiers, u16, u16)],
        #[case] expected: Option<Selection>,
    ) {
        let session = pinned_session(6, &[3]);
        let mut ui = ui_state();

        handle_all(&session, &mut ui, events);

        assert_eq!(ui.selection, expected);
    }

    #[test]
    fn handle_click_outside_rows_clears_selection() {
        let session = pinned_session(6, &[3]);
        let mut ui = ui_state();
        ui.selection = Some(selection(0..=0, 0..=0));

        handle_all(&session, &mut ui, &[(LEFT_DOWN, KeyModifiers::NONE, 30, 1)]);

        assert_eq!(ui.selection, None);
    }

    #[rstest]
    #[case::middle_drag(&[(MIDDLE_DOWN, KeyModifiers::NONE, 2, 0), (MIDDLE_DRAG, KeyModifiers::NONE, 4, 0)], &[Command::ScrollLeft { amount: 2 }])]
    #[case::after_release(&[(MIDDLE_DOWN, KeyModifiers::NONE, 2, 0), (MIDDLE_UP, KeyModifiers::NONE, 2, 0), (MIDDLE_DRAG, KeyModifiers::NONE, 4, 0)], &[])]
    fn handle_pans(
        #[case] events: &[(MouseEventKind, KeyModifiers, u16, u16)],
        #[case] expected: &[Command],
    ) {
        let session = pinned_session(6, &[3]);
        let mut ui = ui_state();

        assert_eq!(handle_all(&session, &mut ui, events), expected);
    }

    #[test]
    fn handle_gff_hover_sets_tooltip_until_mouse_leaves() {
        let session = session(&[&[b'A'; 100]]);
        let gff = Gff {
            features: vec![Feature {
                name: "gene1".to_string(),
                kind: FeatureType::Gene,
                range: 0..100,
                strand: Strand::Forward,
            }],
        };
        let mut ui = ui_state();
        let screen = gff_screen();
        let mut tracker = MouseTracker::default();
        let gff_rows = screen.app.gff_pane_rows;
        let alignment_rows = screen.app.alignment_pane_sequence_rows;
        let mut handle = |x, y| {
            handle(
                &mut tracker,
                Some(&session),
                Some(&gff),
                &mut ui,
                &screen,
                mouse_event(MouseEventKind::Moved, x, y),
            );
            ui.gff_tooltip.is_some()
        };

        assert!(handle(gff_rows.x, gff_rows.y));
        assert!(!handle(alignment_rows.x, alignment_rows.y));
    }

    #[test]
    fn handle_gff_click_jumps() {
        let session = session(&[&[b'A'; 100]]);
        let gff = Gff {
            features: vec![Feature {
                name: "gene1".to_string(),
                kind: FeatureType::Gene,
                range: 0..100,
                strand: Strand::Forward,
            }],
        };
        let mut ui = ui_state();
        let screen = gff_screen();
        let mut tracker = MouseTracker::default();
        let area = screen.app.gff_pane_rows;

        let commands = handle(
            &mut tracker,
            Some(&session),
            Some(&gff),
            &mut ui,
            &screen,
            mouse_event(LEFT_DOWN, area.x + 1, area.y),
        );

        assert!(matches!(commands.as_slice(), [Command::JumpToIndex(_)]));
    }

    const TRACK_AREA: Rect = Rect::new(10, 0, 10, 1);

    #[rstest]
    #[case::first_cell(10, 100, 0)]
    #[case::scaled(15, 100, 50)]
    #[case::last_cell(19, 100, 90)]
    #[case::clamped_to_last_column(19, 5, 4)]
    #[case::left_of_area(5, 100, 0)]
    fn column_at_works(#[case] x: u16, #[case] total_columns: usize, #[case] expected: usize) {
        assert_eq!(column_at(x, TRACK_AREA, total_columns), expected);
    }

    #[rstest]
    #[case::press_centres(LEFT_DOWN, 15, Some(45))]
    #[case::drag_centres(LEFT_DRAG, 12, Some(15))]
    #[case::near_start_clamps_to_zero(LEFT_DOWN, 10, Some(0))]
    #[case::release_ignored(LEFT_UP, 15, None)]
    #[case::right_click_ignored(MouseEventKind::Down(MouseButton::Right), 15, None)]
    fn centre_on_pointer_works(
        #[case] kind: MouseEventKind,
        #[case] x: u16,
        #[case] expected: Option<usize>,
    ) {
        assert_eq!(
            centre_on_pointer(mouse_event(kind, x, 0), TRACK_AREA, 10, 100),
            expected.map(Command::JumpToIndex)
        );
    }
}
