use crate::{
    config::theme::{SequenceStyles, Theme, ThemeId, ThemeStyles},
    core::session::Position,
    ui::{
        bars::{Notification, NotificationLevel},
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
    notification: Option<Notification>,
    pub selection: Option<Selection>,
    pub theme: ThemeState,
    pub position: Position,
    pub loading: Option<String>,
    pub gff_tooltip: Option<String>,
}

impl UiState {
    pub fn notification(&self) -> Option<&Notification> {
        self.notification.as_ref()
    }

    pub fn notify(&mut self, notification: Notification) {
        let error_shown = self
            .notification
            .as_ref()
            .is_some_and(|shown| shown.level() == NotificationLevel::Error);
        if notification.level() == NotificationLevel::Error || !error_shown {
            self.notification = Some(notification);
        }
    }

    pub fn clear_notification(&mut self) {
        self.notification = None;
    }

    pub fn set_theme(&mut self, theme_id: ThemeId) {
        if self.theme.id != theme_id {
            self.theme = ThemeState::new(theme_id);
        }
    }

    pub fn clear_transient_state(&mut self) {
        self.selection = None;
        self.popup = None;
        self.notification = None;
        self.gff_tooltip = None;
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::first(None, Notification::info(""), NotificationLevel::Info)]
    #[case::info_replaces_info(
        Some(Notification::info("")),
        Notification::info(""),
        NotificationLevel::Info
    )]
    #[case::error_replaces_info(
        Some(Notification::info("")),
        Notification::error(""),
        NotificationLevel::Error
    )]
    #[case::info_keeps_error(
        Some(Notification::error("")),
        Notification::info(""),
        NotificationLevel::Error
    )]
    fn notify_works(
        #[case] shown: Option<Notification>,
        #[case] incoming: Notification,
        #[case] expected: NotificationLevel,
    ) {
        let mut ui = UiState::default();
        if let Some(shown) = shown {
            ui.notify(shown);
        }

        ui.notify(incoming);

        assert_eq!(ui.notification().map(Notification::level), Some(expected));
    }
}
