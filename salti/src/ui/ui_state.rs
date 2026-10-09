use crate::{
    config::theme::{SequenceStyles, Theme, ThemeId, ThemeStyles},
    core::session::Position,
    ui::{
        layers::{
            notification::{Notification, NotificationLevel},
            state::LayerState,
        },
        panes::gff::GffPaneState,
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
    pub(crate) layers: LayerState,
    pub(crate) gff_pane: GffPaneState,
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
            .is_some_and(|shown| shown.level == NotificationLevel::Error);
        if notification.level == NotificationLevel::Error || !error_shown {
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
        self.layers.close_active();
        self.notification = None;
        self.gff_tooltip = None;
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn notification(level: NotificationLevel) -> Notification {
        Notification {
            level,
            message: String::new(),
        }
    }

    #[rstest]
    #[case::first(None, NotificationLevel::Info, NotificationLevel::Info)]
    #[case::info_replaces_info(
        Some(NotificationLevel::Info),
        NotificationLevel::Info,
        NotificationLevel::Info
    )]
    #[case::error_replaces_info(
        Some(NotificationLevel::Info),
        NotificationLevel::Error,
        NotificationLevel::Error
    )]
    #[case::info_keeps_error(
        Some(NotificationLevel::Error),
        NotificationLevel::Info,
        NotificationLevel::Error
    )]
    fn notify_works(
        #[case] shown: Option<NotificationLevel>,
        #[case] level: NotificationLevel,
        #[case] expected: NotificationLevel,
    ) {
        let mut ui = UiState::default();
        if let Some(shown) = shown {
            ui.notify(notification(shown));
        }

        ui.notify(notification(level));

        assert_eq!(ui.notification().map(|shown| shown.level), Some(expected));
    }
}
