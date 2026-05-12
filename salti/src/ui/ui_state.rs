use crate::{
    cli::StartupState,
    config::theme::{
        EVERFOREST_DARK, Theme, ThemeId, ThemeStyles, build_theme_styles, theme_from_id,
    },
    core::Viewport,
    ui::{layers::notification::Notification, layers::state::LayerState, panes::gff::GffPaneState},
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
    Failed(String),
}

impl std::fmt::Display for LoadingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Status: Idle"),
            Self::Loading => write!(f, "Status: Loading"),
            Self::Loaded => write!(f, "Status: Loaded"),
            Self::Failed(_) => write!(f, "Status: Failed"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaState {
    pub loading_state: LoadingState,
    pub input_path: Option<String>,
    pub initial_position: usize,
}

impl From<StartupState> for MetaState {
    fn from(startup: StartupState) -> Self {
        Self {
            loading_state: LoadingState::Idle,
            input_path: startup.file_path,
            initial_position: startup.initial_position,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseSelection {
    pub sequence_id: usize,
    pub column: usize,
    pub end_sequence_id: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeState {
    pub id: ThemeId,
    pub theme: Theme,
    pub styles: ThemeStyles,
}

impl Default for ThemeState {
    fn default() -> Self {
        let id = ThemeId::EverforestDark;
        let theme = EVERFOREST_DARK;
        let styles = build_theme_styles(theme);
        Self { id, theme, styles }
    }
}

#[derive(Debug)]
pub struct UiState {
    pub(crate) layers: LayerState,
    pub(crate) gff_pane: GffPaneState,
    pub notification: Option<Notification>,
    pub selection: Option<MouseSelection>,
    pub theme: ThemeState,
    pub viewport: Viewport,
    pub meta: MetaState,
    pub gff_tooltip: Option<String>,
}

impl UiState {
    pub fn new(startup: StartupState) -> Self {
        Self {
            layers: LayerState::default(),
            gff_pane: GffPaneState::default(),
            notification: None,
            selection: None,
            theme: ThemeState::default(),
            viewport: Viewport::default(),
            meta: MetaState::from(startup),
            gff_tooltip: None,
        }
    }

    pub fn set_theme(&mut self, theme_id: ThemeId) {
        if self.theme.id != theme_id {
            self.theme.id = theme_id;
            self.theme.theme = theme_from_id(theme_id);
            self.theme.styles = build_theme_styles(self.theme.theme);
        }
    }

    pub fn clear_transient_state(&mut self) {
        self.selection = None;
        self.layers.close_active();
        self.notification = None;
    }
}
