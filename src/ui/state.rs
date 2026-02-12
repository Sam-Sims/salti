use crate::config::theme::{
    EVERFOREST_DARK, Theme, ThemeId, ThemeStyles, build_theme_styles, theme_from_id,
};
use crate::core::{CoreState, VisibleSequence};
use crate::overlay::{CommandPaletteState, OverlayState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseSelection {
    pub sequence_id: usize,
    pub column: usize,
    pub end_sequence_id: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone)]
pub enum UiAction {
    OpenCommandPalette,
    CloseCommandPalette,
    ShowCommandError(String),
    ClearCommandError,
    SetTheme(ThemeId),
    SetMouseSelection(MouseSelection),
    ClearMouseSelection,
}

#[derive(Debug)]
pub struct UiState {
    pub overlay: OverlayState,
    pub theme_id: ThemeId,
    pub theme: Theme,
    pub theme_styles: ThemeStyles,
    pub mouse_selection: Option<MouseSelection>,
}

impl Default for UiState {
    fn default() -> Self {
        let theme_id = ThemeId::EverforestDark;
        let theme = EVERFOREST_DARK;
        let theme_styles = build_theme_styles(theme);
        Self {
            overlay: OverlayState::default(),
            theme_id,
            theme,
            theme_styles,
            mouse_selection: None,
        }
    }
}

impl UiState {
    pub fn apply_action(&mut self, action: UiAction, core: &CoreState) {
        match action {
            UiAction::OpenCommandPalette => {
                self.overlay.command_error = None;
                let selectable_sequences: Vec<VisibleSequence> = core
                    .visible_sequences()
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
                ));
            }
            UiAction::CloseCommandPalette => {
                self.overlay.palette = None;
            }
            UiAction::ShowCommandError(message) => {
                self.overlay.command_error = Some(message);
            }
            UiAction::ClearCommandError => {
                self.overlay.command_error = None;
            }
            UiAction::SetTheme(theme_id) => {
                if self.theme_id != theme_id {
                    self.theme_id = theme_id;
                    self.theme = theme_from_id(theme_id);
                    self.theme_styles = build_theme_styles(self.theme);
                }
            }
            UiAction::SetMouseSelection(mouse_selection) => {
                self.mouse_selection = Some(mouse_selection);
            }
            UiAction::ClearMouseSelection => {
                self.mouse_selection = None;
            }
        }
    }
}
