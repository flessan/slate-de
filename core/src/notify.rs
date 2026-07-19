//! In-session notifications.
//!
//! Slate intentionally does **not** shell out to `libnotify` by default —
//! notifications are first-class data inside the desktop (counted on the
//! status bar, listed by the `notifications` command, shown in the logs
//! pane). Bridging to org.freedesktop.Notifications is planned for v0.3.

use crate::civil::DateTime;

/// Severity of a notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// A single notification entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub id: u64,
    /// UTC timestamp (unix seconds).
    pub ts: i64,
    pub level: Level,
    pub text: String,
}

impl Notification {
    pub fn new(id: u64, level: Level, text: impl Into<String>) -> Self {
        Notification { id, ts: DateTime::now_utc().secs, level, text: text.into() }
    }

    /// `14:03` style short timestamp for list display.
    pub fn time_label(&self) -> String {
        DateTime::from_unix(self.ts).format_time()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_basics() {
        let n = Notification::new(7, Level::Warn, "battery low");
        assert_eq!(n.id, 7);
        assert_eq!(n.level.label(), "warn");
        assert!(n.time_label().contains(':'));
    }
}
