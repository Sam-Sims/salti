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
    use crate::{
        cli::StartupState,
        core::gff::{Feature, FeatureType, Strand},
        ui::{
            layers::palette::CommandPaletteState,
            layout::{AlignmentHeaderLayout, AppLayout, FrameLayout},
        },
    };

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn ui_state() -> UiState {
        UiState::new(StartupState {
            file_path: None,
            initial_position: 0,
        })
    }

    fn alignment_model(sequences: Vec<libmsa::RawSequence>) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(sequences).expect("alignment should be valid");
        AlignmentModel::new(alignment).expect("alignment model should be valid")
    }

    fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn left_mouse_event(
        kind: MouseEventKind,
        area: Rect,
        column_offset: u16,
        row_offset: u16,
    ) -> MouseEvent {
        mouse_event(kind, area.x + column_offset, area.y + row_offset)
    }

    fn feature(name: &str, start: usize, end: usize) -> Feature {
        Feature {
            name: name.to_string(),
            kind: FeatureType::Gene,
            range: start..end,
            strand: Strand::Forward,
        }
    }

    #[test]
    fn palette_route_masks_mouse_commands() {
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        ui.layers.open_palette(CommandPaletteState::empty());
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );
        let mouse = mouse_event(MouseEventKind::Down(MouseButton::Left), 10, 10);

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
        let model = alignment_model(vec![
            raw("row1", sequence_a.as_slice()),
            raw("row2", sequence_c.as_slice()),
        ]);
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );
        ui.viewport.update_dimensions(78, 10, 20);
        ui.viewport.set_bounds(2, 200, 4);
        ui.layers.toggle_minimap();
        let mouse = mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            frame_layout.overlay_area.x + frame_layout.overlay_area.width - 2,
            frame_layout.overlay_area.y + frame_layout.overlay_area.height - 2,
        );

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
    fn translated_click_selects_codon() {
        let mut model = alignment_model(vec![raw("seq1", b"ATGAAATTT"), raw("seq2", b"ATGAAATTT")]);
        model
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .expect("translation should succeed");
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );
        ui.viewport.update_dimensions(60, 10, 20);
        ui.viewport.set_bounds(2, 9, 4);

        let mouse = left_mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            app_layout.alignment_pane_sequence_rows,
            1,
            0,
        );
        handle_mouse_event(
            &mut tracker,
            Some(&model),
            None,
            &mut ui,
            &frame_layout,
            &app_layout,
            mouse,
        );

        assert_eq!(
            ui.selection,
            Some(MouseSelection {
                sequence_id: 0,
                column: 0,
                end_sequence_id: 0,
                end_column: 2,
            })
        );
    }

    #[test]
    fn translated_ctrl_drag_spans_codons() {
        let mut model = alignment_model(vec![raw("seq1", b"ATGAAATTT"), raw("seq2", b"ATGAAATTT")]);
        model
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .expect("translation should succeed");
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );
        ui.viewport.update_dimensions(60, 10, 20);
        ui.viewport.set_bounds(2, 9, 4);
        let area = app_layout.alignment_pane_sequence_rows;

        for mouse in [
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: area.x + 1,
                row: area.y,
                modifiers: KeyModifiers::CONTROL,
            },
            left_mouse_event(MouseEventKind::Drag(MouseButton::Left), area, 7, 0),
            left_mouse_event(MouseEventKind::Up(MouseButton::Left), area, 7, 0),
        ] {
            handle_mouse_event(
                &mut tracker,
                Some(&model),
                None,
                &mut ui,
                &frame_layout,
                &app_layout,
                mouse,
            );
        }

        assert_eq!(
            ui.selection,
            Some(MouseSelection {
                sequence_id: 0,
                column: 0,
                end_sequence_id: 0,
                end_column: 8,
            })
        );
    }

    #[test]
    fn left_click_outside_msa_clears_selection() {
        let model = alignment_model(vec![raw("seq1", b"ATG"), raw("seq2", b"ATG")]);
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        ui.selection = Some(MouseSelection {
            sequence_id: 0,
            column: 0,
            end_sequence_id: 0,
            end_column: 0,
        });
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );
        ui.viewport.update_dimensions(60, 10, 20);
        ui.viewport.set_bounds(2, 3, 4);

        handle_mouse_event(
            &mut tracker,
            Some(&model),
            None,
            &mut ui,
            &frame_layout,
            &app_layout,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 0, 0),
        );

        assert_eq!(ui.selection, None);
    }

    #[test]
    fn middle_drag_pans_msa() {
        let model = alignment_model(vec![raw("seq1", b"ATG"), raw("seq2", b"ATG")]);
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            0,
            AlignmentHeaderLayout::without_features(),
        );
        let area = app_layout.alignment_pane_sequence_rows;

        handle_mouse_event(
            &mut tracker,
            Some(&model),
            None,
            &mut ui,
            &frame_layout,
            &app_layout,
            left_mouse_event(MouseEventKind::Down(MouseButton::Middle), area, 10, 5),
        );
        let commands = handle_mouse_event(
            &mut tracker,
            Some(&model),
            None,
            &mut ui,
            &frame_layout,
            &app_layout,
            left_mouse_event(MouseEventKind::Drag(MouseButton::Middle), area, 12, 7),
        );

        assert_eq!(
            commands,
            vec![
                Command::ScrollUp { amount: 2 },
                Command::ScrollLeft { amount: 2 }
            ]
        );
    }

    #[test]
    fn gff_hover_sets_tooltip() {
        let model = alignment_model(vec![raw("seq1", &[b'A'; 100])]);
        let gff = Gff {
            features: vec![feature("gene1", 0, 100)],
        };
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            4,
            AlignmentHeaderLayout::without_features(),
        );
        ui.viewport.update_dimensions(60, 10, 20);
        ui.viewport.set_bounds(1, 100, 4);
        let mouse = mouse_event(
            MouseEventKind::Moved,
            app_layout.gff_pane_rows.x,
            app_layout.gff_pane_rows.y,
        );

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
        assert!(
            ui.gff_tooltip
                .as_deref()
                .is_some_and(|it| it.starts_with("gene1 "))
        );
    }

    #[test]
    fn gff_drag_emits_jump() {
        let model = alignment_model(vec![raw("seq1", &[b'A'; 100])]);
        let gff = Gff {
            features: vec![feature("gene1", 0, 100)],
        };
        let mut tracker = MouseTracker::default();
        let mut ui = ui_state();
        let frame_layout = FrameLayout::new(Rect::new(0, 0, 80, 24));
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            4,
            AlignmentHeaderLayout::without_features(),
        );
        ui.viewport.update_dimensions(60, 10, 20);
        ui.viewport.set_bounds(1, 100, 4);
        let area = app_layout.gff_pane_rows;

        handle_mouse_event(
            &mut tracker,
            Some(&model),
            Some(&gff),
            &mut ui,
            &frame_layout,
            &app_layout,
            mouse_event(MouseEventKind::Down(MouseButton::Left), area.x + 1, area.y),
        );
        let commands = handle_mouse_event(
            &mut tracker,
            Some(&model),
            Some(&gff),
            &mut ui,
            &frame_layout,
            &app_layout,
            mouse_event(
                MouseEventKind::Drag(MouseButton::Left),
                area.x + area.width - 1,
                area.y,
            ),
        );

        assert!(matches!(commands.as_slice(), [Command::JumpToPosition(_)]));
    }
}
