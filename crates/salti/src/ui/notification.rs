use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use tachyonfx::{Effect, fx};

const FADE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notification {
    pub(crate) level: NotificationLevel,
    pub(crate) message: String,
}

impl Notification {
    pub(crate) fn info(message: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Info,
            message: message.into(),
        }
    }

    pub(crate) fn error(message: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Error,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationLevel {
    Error,
    #[allow(dead_code)]
    Warning,
    Info,
}

impl NotificationLevel {
    fn hold(self) -> Duration {
        match self {
            NotificationLevel::Error => Duration::from_secs(3),
            NotificationLevel::Warning | NotificationLevel::Info => Duration::from_secs(1),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Notifications {
    shown: Option<Shown>,
}

impl Notifications {
    pub(crate) fn notify(&mut self, notification: Notification, bg: Color) {
        let error_shown = self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.notification.level == NotificationLevel::Error);
        if error_shown && notification.level != NotificationLevel::Error {
            return;
        }
        let out = match bg {
            Color::Rgb(..) => fx::fade_to_fg(bg, FADE),
            _ => fx::dissolve(FADE),
        };
        self.shown = Some(Shown {
            effect: fx::sequence(&[fx::sleep(notification.level.hold()), out]),
            notification,
        });
    }

    pub(crate) fn shown(&self) -> Option<&Notification> {
        self.shown.as_ref().map(|shown| &shown.notification)
    }

    pub(crate) fn advance(&mut self, delta: Duration, buf: &mut Buffer, area: Rect) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        shown.effect.process(delta, buf, area);
        if shown.effect.done() {
            self.shown = None;
        }
    }

    pub(crate) fn is_animating(&self) -> bool {
        self.shown.is_some()
    }
}

#[derive(Debug)]
struct Shown {
    notification: Notification,
    effect: Effect,
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
        let mut notifications = Notifications::default();
        if let Some(shown) = shown {
            notifications.notify(shown, Color::Reset);
        }

        notifications.notify(incoming, Color::Reset);

        assert_eq!(
            notifications.shown().map(|notification| notification.level),
            Some(expected)
        );
    }

    #[rstest]
    #[case::info(Notification::info("x"), Duration::from_millis(2900))]
    #[case::error(Notification::error("x"), Duration::from_millis(4900))]
    fn advance_clears_after_fade(
        #[case] notification: Notification,
        #[case] shown_until: Duration,
    ) {
        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        let mut notifications = Notifications::default();
        notifications.notify(notification, Color::Rgb(0, 0, 0));

        notifications.advance(shown_until, &mut buf, area);
        assert!(notifications.shown().is_some());

        notifications.advance(Duration::from_millis(100), &mut buf, area);
        assert!(notifications.shown().is_none());
    }
}
