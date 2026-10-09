use std::ops::Range;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::{
    command::Command,
    core::{gff::Gff, session::Session},
    input::route::{MouseRoute, route_mouse},
    ui::{
        layers::{minimap::MinimapState, state::ActiveLayer},
        layout::{AppLayout, FrameLayout, Window, screen_rows},
        panes::gff,
        selection::Selection,
        ui_state::UiState,
    },
};

#[derive(Debug, Default)]
pub(crate) struct MouseTracker {
    box_anchor: Option<Selection>,
    pan_anchor: Option<(u16, u16)>,
}

impl MouseTracker {
    pub fn clear_anchors(&mut self) {
        self.box_anchor = None;
        self.pan_anchor = None;
    }

    fn pan_drag_commands(&mut self, column: u16, row: u16) -> [Option<Command>; 2] {
        let Some((anchor_x, anchor_y)) = self.pan_anchor else {
            return [None, None];
        };

        let (dy_amount, scroll_up) = if row >= anchor_y {
            (usize::from(row - anchor_y), true)
        } else {
            (usize::from(anchor_y - row), false)
        };

        let (dx_amount, scroll_left) = if column >= anchor_x {
            (usize::from(column - anchor_x), true)
        } else {
            (usize::from(anchor_x - column), false)
        };

        self.pan_anchor = Some((column, row));

        [
            (dy_amount > 0).then_some(if scroll_up {
                Command::ScrollUp { amount: dy_amount }
            } else {
                Command::ScrollDown { amount: dy_amount }
            }),
            (dx_amount > 0).then_some(if scroll_left {
                Command::ScrollLeft { amount: dx_amount }
            } else {
                Command::ScrollRight { amount: dx_amount }
            }),
        ]
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_mouse_event(
    tracker: &mut MouseTracker,
    session: Option<&Session>,
    gff: Option<&Gff>,
    ui: &mut UiState,
    frame_layout: &FrameLayout,
    app_layout: &AppLayout,
    mouse: MouseEvent,
) -> Vec<Command> {
    let mut commands = Vec::new();
    ui.gff_tooltip = None;

    match route_mouse(ui, frame_layout, app_layout, mouse, gff.is_some()) {
        MouseRoute::Palette => (),
        MouseRoute::Minimap => {
            if let Some(session) = session
                && let Some(ActiveLayer::Minimap(minimap_state)) = ui.layers.active.as_mut()
            {
                handle_minimap_mouse_event(
                    &mut commands,
                    session,
                    &ui.window.columns,
                    minimap_state,
                    frame_layout,
                    mouse,
                );
            }
        }
        MouseRoute::GffPane => {
            if let (Some(gff), Some(session)) = (gff, session) {
                handle_gff_mouse_event(&mut commands, gff, session, ui, app_layout, mouse);
            }
        }
        MouseRoute::Alignment => {
            if let Some(session) = session {
                handle_alignment_mouse_event(
                    &mut commands,
                    tracker,
                    session,
                    ui,
                    app_layout,
                    mouse,
                );
            }
        }
    }
    commands
}

fn handle_minimap_mouse_event(
    commands: &mut Vec<Command>,
    session: &Session,
    window_columns: &Range<usize>,
    minimap_state: &mut MinimapState,
    frame_layout: &FrameLayout,
    mouse: MouseEvent,
) {
    let total_columns = session.layout().columns().len();
    let overlay_area = frame_layout.overlay_area;

    if let Some(cmd) =
        minimap_state.handle_mouse(mouse, overlay_area, window_columns, total_columns)
    {
        commands.push(cmd);
    }
}

fn handle_gff_mouse_event(
    commands: &mut Vec<Command>,
    gff: &Gff,
    session: &Session,
    ui: &mut UiState,
    app_layout: &AppLayout,
    mouse: MouseEvent,
) {
    let gff_pane_rows = app_layout.gff_pane_rows;
    let total_columns = session.layout().columns().len();

    if let Some(cmd) =
        ui.gff_pane
            .handle_mouse(mouse, gff_pane_rows, &ui.window.columns, total_columns)
    {
        commands.push(cmd);
    }

    ui.gff_tooltip = gff::tooltip_at(gff, session, gff_pane_rows, mouse.column, mouse.row);
}

fn handle_alignment_mouse_event(
    commands: &mut Vec<Command>,
    tracker: &mut MouseTracker,
    session: &Session,
    ui: &mut UiState,
    app_layout: &AppLayout,
    mouse: MouseEvent,
) {
    let resolved_anchor = anchor_at(
        session,
        &ui.window,
        app_layout.alignment_pane_sequence_rows,
        mouse.column,
        mouse.row,
    );

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(anchor) = resolved_anchor else {
                ui.selection = None;
                tracker.clear_anchors();
                return;
            };
            let store_anchor = mouse.modifiers.contains(KeyModifiers::CONTROL);

            tracker.box_anchor = store_anchor.then_some(anchor);
            ui.selection = Some(anchor);
        }
        MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
            if let Some(current) = resolved_anchor {
                let anchor = tracker.box_anchor.unwrap_or(current);
                ui.selection = Some(selection_from_anchors(anchor, current));
            }
            if mouse.kind == MouseEventKind::Up(MouseButton::Left) {
                tracker.clear_anchors();
            }
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            tracker.pan_anchor = Some((mouse.column, mouse.row));
        }
        MouseEventKind::Drag(MouseButton::Middle) => {
            commands.extend(
                tracker
                    .pan_drag_commands(mouse.column, mouse.row)
                    .into_iter()
                    .flatten(),
            );
        }
        MouseEventKind::Up(MouseButton::Middle) => {
            tracker.pan_anchor = None;
        }
        _ => (),
    }
}

fn anchor_at(
    session: &Session,
    window: &Window,
    sequence_rows_area: Rect,
    mouse_x: u16,
    mouse_y: u16,
) -> Option<Selection> {
    if !sequence_rows_area.contains((mouse_x, mouse_y).into()) {
        return None;
    }

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

#[cfg(test)]
mod tests {
    use std::ops;

    use rstest::rstest;

    use super::*;
    use crate::{
        core::{
            gff::{Feature, FeatureType, Strand},
            session::{Position, ViewState},
        },
        test_utils::{mouse_event, pinned_session, session, ui_state},
        ui::{layers::palette::CommandPaletteState, layout::AlignmentHeaderLayout},
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

    fn layouts(gff_height: u16) -> (FrameLayout, AppLayout) {
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            gff_height,
            AlignmentHeaderLayout::without_features(),
        );
        (frame_layout, app_layout)
    }

    fn pinned_window() -> Window {
        Window {
            pinned: 0..1,
            rows: 0..4,
            columns: 0..20,
            ..Window::default()
        }
    }

    fn handle_all(
        session: &Session,
        gff: Option<&Gff>,
        ui: &mut UiState,
        gff_height: u16,
        events: &[(MouseEventKind, KeyModifiers, u16, u16)],
    ) -> Vec<Command> {
        let (frame_layout, app_layout) = layouts(gff_height);
        let area = app_layout.alignment_pane_sequence_rows;
        let mut tracker = MouseTracker::default();
        events
            .iter()
            .flat_map(|&(kind, modifiers, x, y)| {
                let mouse = MouseEvent {
                    modifiers,
                    ..mouse_event(kind, area.x + x, area.y + y)
                };
                handle_mouse_event(
                    &mut tracker,
                    Some(session),
                    gff,
                    ui,
                    &frame_layout,
                    &app_layout,
                    mouse,
                )
            })
            .collect()
    }

    #[rstest]
    #[case::pinned_row(10, 5, Some(selection(0..=0, 5..=5)))]
    #[case::main_row(12, 7, Some(selection(1..=1, 7..=7)))]
    #[case::divider(10, 6, None)]
    #[case::past_window_rows(10, 9, None)]
    #[case::past_window_columns(18, 5, None)]
    #[case::left_of_area(9, 5, None)]
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
        Some(selection(2..=2, 3..=3)),
    )]
    fn handle_mouse_event_selects(
        #[case] events: &[(MouseEventKind, KeyModifiers, u16, u16)],
        #[case] expected: Option<Selection>,
    ) {
        let session = pinned_session(6, &[3]);
        let mut ui = ui_state();
        ui.window = pinned_window();

        handle_all(&session, None, &mut ui, 0, events);

        assert_eq!(ui.selection, expected);
    }

    #[test]
    fn handle_mouse_event_click_outside_rows_clears_selection() {
        let session = pinned_session(6, &[3]);
        let mut ui = ui_state();
        ui.window = pinned_window();
        ui.selection = Some(selection(0..=0, 0..=0));

        handle_all(
            &session,
            None,
            &mut ui,
            0,
            &[(LEFT_DOWN, KeyModifiers::NONE, 30, 1)],
        );

        assert_eq!(ui.selection, None);
    }

    #[test]
    fn handle_mouse_event_palette_ignores_mouse() {
        let session = pinned_session(6, &[3]);
        let mut ui = ui_state();
        ui.window = pinned_window();
        ui.layers.open_palette(CommandPaletteState::empty());

        let commands = handle_all(
            &session,
            None,
            &mut ui,
            0,
            &[
                (LEFT_DOWN, KeyModifiers::NONE, 0, 0),
                (MIDDLE_DOWN, KeyModifiers::NONE, 0, 0),
                (MIDDLE_DRAG, KeyModifiers::NONE, 2, 2),
            ],
        );

        assert_eq!(commands, []);
        assert_eq!(ui.selection, None);
    }

    #[rstest]
    #[case::middle_drag(&[(MIDDLE_DOWN, KeyModifiers::NONE, 2, 0), (MIDDLE_DRAG, KeyModifiers::NONE, 4, 0)], &[Command::ScrollLeft { amount: 2 }])]
    #[case::after_release(&[(MIDDLE_DOWN, KeyModifiers::NONE, 2, 0), (MIDDLE_UP, KeyModifiers::NONE, 2, 0), (MIDDLE_DRAG, KeyModifiers::NONE, 4, 0)], &[])]
    fn handle_mouse_event_pans(
        #[case] events: &[(MouseEventKind, KeyModifiers, u16, u16)],
        #[case] expected: &[Command],
    ) {
        let session = pinned_session(6, &[3]);
        let mut ui = ui_state();
        ui.window = pinned_window();

        assert_eq!(handle_all(&session, None, &mut ui, 0, events), expected);
    }

    #[test]
    fn handle_mouse_event_gff_hover_sets_tooltip_until_mouse_leaves() {
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
        ui.window = Window {
            rows: 0..1,
            columns: 0..60,
            ..Window::default()
        };
        let (frame_layout, app_layout) = layouts(4);
        let mut tracker = MouseTracker::default();
        let gff_rows = app_layout.gff_pane_rows;
        let alignment_rows = app_layout.alignment_pane_sequence_rows;
        let mut handle = |x, y| {
            handle_mouse_event(
                &mut tracker,
                Some(&session),
                Some(&gff),
                &mut ui,
                &frame_layout,
                &app_layout,
                mouse_event(MouseEventKind::Moved, x, y),
            );
            ui.gff_tooltip.is_some()
        };

        assert!(handle(gff_rows.x, gff_rows.y));
        assert!(!handle(alignment_rows.x, alignment_rows.y));
    }

    #[test]
    fn handle_mouse_event_gff_click_jumps() {
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
        ui.window = Window {
            rows: 0..1,
            columns: 0..60,
            ..Window::default()
        };
        let (frame_layout, app_layout) = layouts(4);
        let mut tracker = MouseTracker::default();
        let area = app_layout.gff_pane_rows;

        let commands = handle_mouse_event(
            &mut tracker,
            Some(&session),
            Some(&gff),
            &mut ui,
            &frame_layout,
            &app_layout,
            mouse_event(LEFT_DOWN, area.x + 1, area.y),
        );

        assert!(matches!(commands.as_slice(), [Command::JumpToIndex(_)]));
    }
}
