use crate::config::theme::{
    EVERFOREST_DARK, Theme, ThemeId, ThemeStyles, build_theme_styles, theme_from_id,
};
use crate::core::CoreState;
use crate::core::command::CoreAction;
use crate::core::viewport::ViewportWindow;
use crate::overlay::{CommandPaletteState, MinimapState, Notification, OverlayState};
use crate::ui::VisibleSequence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseSelection {
    pub sequence_id: usize,
    pub column: usize,
    pub end_sequence_id: usize,
    pub end_column: usize,
}

#[derive(Debug, Default)]
pub struct MouseState {
    pub selection: Option<MouseSelection>,
    pub box_anchor: Option<(usize, usize)>,
    pub pan_anchor: Option<(u16, u16)>,
}

impl MouseState {
    pub fn clear_anchors(&mut self) {
        self.box_anchor = None;
        self.pan_anchor = None;
    }

    pub fn clear_all(&mut self) {
        self.selection = None;
        self.clear_anchors();
    }

    pub fn pan_drag_actions(&mut self, column: u16, row: u16) -> [Option<CoreAction>; 2] {
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
            (dy_amount > 0).then(|| {
                if scroll_up {
                    CoreAction::ScrollUp { amount: dy_amount }
                } else {
                    CoreAction::ScrollDown { amount: dy_amount }
                }
            }),
            (dx_amount > 0).then(|| {
                if scroll_left {
                    CoreAction::ScrollLeft { amount: dx_amount }
                } else {
                    CoreAction::ScrollRight { amount: dx_amount }
                }
            }),
        ]
    }
}

#[derive(Debug, Clone)]
pub enum UiAction {
    OpenCommandPalette,
    CloseCommandPalette,
    ToggleMinimap,
    ShowNotification(Notification),
    ClearNotification,
    SetTheme(ThemeId),
}

#[derive(Debug)]
pub struct UiState {
    pub overlay: OverlayState,
    pub mouse: MouseState,
    pub visible_rows: Vec<Option<usize>>,
    pub display_index: Vec<usize>,
    pub theme_id: ThemeId,
    pub theme: Theme,
    pub theme_styles: ThemeStyles,
}

impl Default for UiState {
    fn default() -> Self {
        let theme_id = ThemeId::EverforestDark;
        let theme = EVERFOREST_DARK;
        let theme_styles = build_theme_styles(theme);
        Self {
            overlay: OverlayState::default(),
            mouse: MouseState::default(),
            visible_rows: Vec::new(),
            display_index: Vec::new(),
            theme_id,
            theme,
            theme_styles,
        }
    }
}

impl UiState {
    pub fn rebuild_visible_rows(
        &mut self,
        core: &CoreState,
        window: &ViewportWindow,
        row_capacity: usize,
    ) {
        let row_ids = core.row_visibility.visible_to_absolute();
        let pinned_count = core.visible_pinned_count();
        let has_pins = pinned_count > 0 && row_capacity > 0;
        let pinned_rows = if has_pins {
            pinned_count.min(row_capacity.saturating_sub(1))
        } else {
            0
        };
        let unpinned_rows = row_capacity.saturating_sub(pinned_rows + usize::from(has_pins));
        let scroll_offset = window.row_range.start;

        self.visible_rows.clear();
        self.visible_rows
            .extend(row_ids[..pinned_rows].iter().copied().map(Some));
        if has_pins {
            self.visible_rows.push(None);
        }
        let unpinned_start = pinned_count + scroll_offset;
        let unpinned_end = (unpinned_start + unpinned_rows).min(row_ids.len());
        self.visible_rows.extend(
            row_ids[unpinned_start..unpinned_end]
                .iter()
                .copied()
                .map(Some),
        );

        self.display_index.clear();
        self.display_index.resize(core.data.sequences.len(), 0);
        for (display_index, &sequence_id) in row_ids.iter().enumerate() {
            self.display_index[sequence_id] = display_index;
        }
    }

    pub fn apply_action(&mut self, action: UiAction, core: &CoreState) {
        match action {
            UiAction::OpenCommandPalette => {
                self.overlay.notification = None;
                self.overlay.minimap = None;
                let selectable_sequences: Vec<VisibleSequence> = core
                    .all_visible_sequences()
                    .map(|sequence| VisibleSequence {
                        sequence_id: sequence.sequence_id,
                        sequence_name: sequence.alignment.id.clone(),
                    })
                    .collect();
                let pinned_sequences: Vec<VisibleSequence> = core
                    .pinned_sequences()
                    .map(|sequence| VisibleSequence {
                        sequence_id: sequence.sequence_id,
                        sequence_name: sequence.alignment.id.clone(),
                    })
                    .collect();
                self.overlay.palette = Some(CommandPaletteState::new(
                    selectable_sequences,
                    pinned_sequences,
                    core.sequence_type(),
                ));
            }
            UiAction::CloseCommandPalette => {
                self.overlay.palette = None;
            }
            UiAction::ToggleMinimap => {
                self.overlay.notification = None;
                self.overlay.palette = None;
                self.overlay.minimap = match self.overlay.minimap {
                    Some(_) => None,
                    None => Some(MinimapState::default()),
                };
            }
            UiAction::ShowNotification(notification) => {
                self.overlay.notification = Some(notification);
            }
            UiAction::ClearNotification => {
                self.overlay.notification = None;
            }
            UiAction::SetTheme(theme_id) => {
                if self.theme_id != theme_id {
                    self.theme_id = theme_id;
                    self.theme = theme_from_id(theme_id);
                    self.theme_styles = build_theme_styles(self.theme);
                }
            }
        }
    }
}
