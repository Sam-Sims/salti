use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use tachyonfx::{ColorSpace, Effect, fx};

const FADE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub level: NotificationLevel,
    pub message: String,
}

impl Notification {
    pub fn info(message: String) -> Self {
        Self {
            level: NotificationLevel::Info,
            message,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            level: NotificationLevel::Error,
            message,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Error,
    #[allow(dead_code)]
    Warning,
    Info,
}

impl NotificationLevel {
    // how long the notification stays before it starts to fade
    fn hold(self) -> Duration {
        match self {
            NotificationLevel::Error => Duration::from_secs(3),
            NotificationLevel::Warning | NotificationLevel::Info => Duration::from_secs(1),
        }
    }
}

#[derive(Debug, Default)]
pub struct Notifications {
    shown: Option<Shown>,
}

impl Notifications {
    /// fades into `background` once it's been up long enough
    ///
    /// an error stays until it fades, so a later info or warning can't hide it
    pub fn notify(&mut self, notification: Notification, background: Color) {
        let error_shown = self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.notification.level == NotificationLevel::Error);
        if error_shown && notification.level != NotificationLevel::Error {
            return;
        }
        // terminals without RGB colours dissolve instead
        let fade_out = match background {
            Color::Rgb(..) => fx::fade_to_fg(background, FADE).with_color_space(ColorSpace::Rgb),
            _ => fx::dissolve(FADE),
        };
        self.shown = Some(Shown {
            effect: fx::sequence(&[fx::sleep(notification.level.hold()), fade_out]),
            notification,
        });
    }

    pub fn shown(&self) -> Option<&Notification> {
        self.shown.as_ref().map(|shown| &shown.notification)
    }

    pub fn advance(&mut self, delta: Duration, buf: &mut Buffer, area: Rect) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        shown.effect.process(delta, buf, area);
        if shown.effect.done() {
            self.shown = None;
        }
    }

    pub fn is_animating(&self) -> bool {
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
    #[case::first(None, Notification::info(String::new()), NotificationLevel::Info)]
    #[case::info_replaces_info(
        Some(Notification::info(String::new())),
        Notification::info(String::new()),
        NotificationLevel::Info
    )]
    #[case::error_replaces_info(
        Some(Notification::info(String::new())),
        Notification::error(String::new()),
        NotificationLevel::Error
    )]
    #[case::info_keeps_error(
        Some(Notification::error(String::new())),
        Notification::info(String::new()),
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
    #[case::info(Notification::info("x".to_string()), Duration::from_millis(2900))]
    #[case::error(Notification::error("x".to_string()), Duration::from_millis(4900))]
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
