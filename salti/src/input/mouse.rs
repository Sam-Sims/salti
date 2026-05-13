use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::{
    command::Command,
    core::{gff::Gff, model::AlignmentModel},
    input::route::{MouseRoute, route_mouse},
    ui::{
        layers::{minimap::MinimapState, state::ActiveLayer},
        layout::{AppLayout, FrameLayout},
        panes::gff,
        selection::selection_point_crosshair,
        ui_state::{MouseSelection, UiState},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MouseAnchor {
    sequence_id: usize,
    column: usize,
    end_column: usize,
}

#[derive(Debug, Default)]
pub(crate) struct MouseTracker {
    box_anchor: Option<MouseAnchor>,
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
    alignment: Option<&AlignmentModel>,
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
            if let Some(alignment) = alignment {
                let viewport_col_range = ui.viewport.window().col_range;
                if let Some(ActiveLayer::Minimap(minimap_state)) = ui.layers.active.as_mut() {
                    handle_minimap_mouse_event(
                        &mut commands,
                        alignment,
                        viewport_col_range,
                        minimap_state,
                        frame_layout,
                        mouse,
                    );
                }
            }
        }
        MouseRoute::GffPane => {
            if let (Some(gff), Some(alignment)) = (gff, alignment) {
                handle_gff_mouse_event(&mut commands, gff, alignment, ui, app_layout, mouse);
            }
        }
        MouseRoute::Alignment => {
            if let Some(alignment) = alignment {
                handle_alignment_mouse_event(
                    &mut commands,
                    tracker,
                    alignment,
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
    alignment: &AlignmentModel,
    viewport_col_range: std::ops::Range<usize>,
    minimap_state: &mut MinimapState,
    frame_layout: &FrameLayout,
    mouse: MouseEvent,
) {
    let total_columns = alignment.view().column_count();
    let overlay_area = frame_layout.overlay_area;

    if let Some(cmd) =
        minimap_state.handle_mouse(mouse, overlay_area, &viewport_col_range, total_columns)
    {
        commands.push(cmd);
    }
}

fn handle_gff_mouse_event(
    commands: &mut Vec<Command>,
    gff: &Gff,
    alignment: &AlignmentModel,
    ui: &mut UiState,
    app_layout: &AppLayout,
    mouse: MouseEvent,
) {
    let viewport_col_range = ui.viewport.window().col_range;
    let gff_pane_rows = app_layout.gff_pane_rows;

    if let Some(cmd) =
        ui.gff_pane
            .handle_mouse(mouse, gff_pane_rows, &viewport_col_range, alignment)
    {
        commands.push(cmd);
    }

    ui.gff_tooltip = gff::tooltip_at(gff, alignment, gff_pane_rows, mouse.column, mouse.row);
}

fn handle_alignment_mouse_event(
    commands: &mut Vec<Command>,
    tracker: &mut MouseTracker,
    alignment: &AlignmentModel,
    ui: &mut UiState,
    app_layout: &AppLayout,
    mouse: MouseEvent,
) {
    let crosshair = selection_point_crosshair(
        alignment,
        &ui.viewport,
        app_layout.alignment_pane_sequence_rows,
        mouse.column,
        mouse.row,
    );
    let resolved_anchor = crosshair
        .and_then(|(sequence_id, column)| anchor_from_crosshair(alignment, sequence_id, column));

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(anchor) = resolved_anchor else {
                ui.selection = None;
                tracker.clear_anchors();
                return;
            };
            let store_anchor = mouse.modifiers.contains(KeyModifiers::CONTROL);

            tracker.box_anchor = if store_anchor { Some(anchor) } else { None };
            ui.selection = Some(selection_from_anchors(anchor, anchor));
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            let Some(current) = resolved_anchor else {
                return;
            };
            let anchor = tracker.box_anchor.unwrap_or(current);
            ui.selection = Some(selection_from_anchors(anchor, current));
        }
        MouseEventKind::Up(MouseButton::Left) => {
            let Some(current) = resolved_anchor else {
                tracker.clear_anchors();
                return;
            };
            let anchor = tracker.box_anchor.unwrap_or(current);
            ui.selection = Some(selection_from_anchors(anchor, current));
            tracker.clear_anchors();
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

fn anchor_from_crosshair(
    alignment: &AlignmentModel,
    sequence_id: usize,
    column: usize,
) -> Option<MouseAnchor> {
    let Some(overlay) = alignment.translation_overlay() else {
        return Some(MouseAnchor {
            sequence_id,
            column,
            end_column: column,
        });
    };
    let codon_span = overlay.codon_span(column)?;
    Some(MouseAnchor {
        sequence_id,
        column: codon_span.start,
        end_column: codon_span.end - 1,
    })
}

fn selection_from_anchors(anchor: MouseAnchor, current: MouseAnchor) -> MouseSelection {
    let (column, end_column) = if current.column < anchor.column {
        (anchor.end_column, current.column)
    } else {
        (anchor.column, current.end_column)
    };

    MouseSelection {
        sequence_id: anchor.sequence_id,
        column,
        end_sequence_id: current.sequence_id,
        end_column,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    use super::*;
    use crate::cli::StartupState;
    use crate::ui::layers::palette::CommandPaletteState;
    use crate::ui::layout::{AppLayout, FrameLayout};

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn gff(start: usize, end: usize) -> Gff {
        Gff {
            features: vec![crate::core::gff::Feature {
                name: "gene".to_string(),
                kind: crate::core::gff::FeatureType::Gene,
                range: start..end,
                strand: crate::core::gff::Strand::Forward,
            }],
        }
    }

    fn ui_state() -> UiState {
        UiState::new(StartupState {
            file_path: None,
            initial_position: 0,
        })
    }

    #[test]
    fn palette_route_masks_mouse_commands() {
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        ui.layers.open_palette(CommandPaletteState::empty());
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(frame_layout.content_area, 0);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 10,
            modifiers: KeyModifiers::empty(),
        };

        let commands = handle_mouse_event(
            &mut tracker,
            None,
            None,
            &mut ui,
            &frame_layout,
            &app_layout,
            mouse,
        );

        assert!(commands.is_empty());
    }

    #[test]
    fn minimap_route_emits_jump_command() {
        let sequence_a = vec![b'A'; 200];
        let sequence_c = vec![b'C'; 200];
        let alignment = libmsa::Alignment::new(vec![
            raw("row1", sequence_a.as_slice()),
            raw("row2", sequence_c.as_slice()),
        ])
        .expect("alignment should be valid");
        let model = crate::core::model::AlignmentModel::new(alignment)
            .expect("alignment model should be valid");
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(frame_layout.content_area, 0);
        ui.viewport.update_dimensions(78, 10, 20);
        ui.viewport.set_bounds(2, 200, 4);
        ui.layers.toggle_minimap();
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: frame_layout.overlay_area.x + frame_layout.overlay_area.width - 2,
            row: frame_layout.overlay_area.y + frame_layout.overlay_area.height - 2,
            modifiers: KeyModifiers::empty(),
        };

        let commands = handle_mouse_event(
            &mut tracker,
            Some(&model),
            None,
            &mut ui,
            &frame_layout,
            &app_layout,
            mouse,
        );

        assert!(matches!(commands.as_slice(), [Command::JumpToPosition(_)]));
    }

    #[test]
    fn moved_event_in_gff_pane_sets_tooltip_without_mouse_down() {
        let alignment = libmsa::Alignment::new(vec![raw("row1", &[b'A'; 20])])
            .expect("alignment should be valid");
        let model = crate::core::model::AlignmentModel::new(alignment)
            .expect("alignment model should be valid");
        let gff = gff(0, 10);
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(frame_layout.content_area, 4);
        ui.viewport.update_dimensions(20, 10, 20);
        ui.viewport.set_bounds(1, 20, 4);
        let mouse = MouseEvent {
            kind: MouseEventKind::Moved,
            column: app_layout.gff_pane_rows.x,
            row: app_layout.gff_pane_rows.y,
            modifiers: KeyModifiers::empty(),
        };

        let commands = handle_mouse_event(
            &mut tracker,
            Some(&model),
            Some(&gff),
            &mut ui,
            &frame_layout,
            &app_layout,
            mouse,
        );

        assert!(commands.is_empty());
        assert_eq!(
            ui.gff_tooltip.as_deref(),
            Some("gene (gene) — Forward →\n1-10 • 10 nt")
        );
    }
}
