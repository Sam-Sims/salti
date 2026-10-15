use ratatui::style::Color;

use crate::{
    config::theme::{Theme, ThemeChoice, ThemeName},
    session::Position,
    ui::{
        notification::{Notification, Notifications},
        popups::Popup,
        selection::Selection,
    },
};

#[derive(Debug, Default)]
pub struct UiState {
    pub(crate) popup: Option<Popup>,
    pub(crate) notifications: Notifications,
    pub selection: Option<Selection>,
    pub theme: Theme,
    pub auto_theme: Option<ThemeName>,
    pub position: Position,
    pub loading: Option<String>,
    pub gff_tooltip: Option<String>,
}

impl UiState {
    pub fn notify(&mut self, notification: Notification) {
        let input_bg = self.theme.base.bg.unwrap_or(Color::Reset);
        self.notifications.notify(notification, input_bg);
    }

    pub fn set_theme(&mut self, choice: ThemeChoice) {
        self.theme = Theme::builtin(choice.resolve(self.auto_theme));
        if choice == ThemeChoice::Auto && self.auto_theme.is_none() {
            self.notify(Notification::error(
                "Could not detect the terminal background. Using a dark theme",
            ));
        }
    }

    pub fn clear_transient_state(&mut self) {
        self.selection = None;
        self.popup = None;
        self.notifications = Notifications::default();
        self.gff_tooltip = None;
    }
}
