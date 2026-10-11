use ratatui::style::Color;

use crate::{
    config::theme::{SequenceStyles, Theme, ThemeId, ThemeStyles},
    core::session::Position,
    ui::{
        notification::{Notification, Notifications},
        popups::Popup,
        selection::Selection,
    },
};

#[derive(Debug, Clone)]
pub struct ThemeState {
    pub id: ThemeId,
    pub theme: Theme,
    pub styles: ThemeStyles,
    pub sequence: Box<SequenceStyles>,
}

impl ThemeState {
    pub fn new(id: ThemeId) -> Self {
        let theme = id.theme();
        Self {
            id,
            theme,
            styles: ThemeStyles::new(&theme),
            sequence: Box::new(SequenceStyles::new(&theme.sequence)),
        }
    }
}

impl Default for ThemeState {
    fn default() -> Self {
        Self::new(ThemeId::EverforestDark)
    }
}

#[derive(Debug, Default)]
pub struct UiState {
    pub(crate) popup: Option<Popup>,
    pub(crate) notifications: Notifications,
    pub selection: Option<Selection>,
    pub theme: ThemeState,
    pub position: Position,
    pub loading: Option<String>,
    pub gff_tooltip: Option<String>,
}

impl UiState {
    pub fn notify(&mut self, notification: Notification) {
        let input_bg = self.theme.styles.base_block.bg.unwrap_or(Color::Reset);
        self.notifications.notify(notification, input_bg);
    }

    pub fn set_theme(&mut self, theme_id: ThemeId) {
        if self.theme.id != theme_id {
            self.theme = ThemeState::new(theme_id);
        }
    }

    pub fn clear_transient_state(&mut self) {
        self.selection = None;
        self.popup = None;
        self.notifications = Notifications::default();
        self.gff_tooltip = None;
    }
}
