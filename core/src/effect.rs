//! Side effects produced by command execution.
//!
//! Commands never touch the terminal directly. They return text plus a list
//! of [`Effect`]s; the shell interprets those (re-layout, workspace switch,
//! theme change, quit…). This is the seam that keeps the command layer
//! testable headlessly.

/// A request from a command to the surrounding shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Repaint the whole screen.
    Redraw,
    /// Switch to the named workspace.
    SwitchWorkspace(String),
    /// Cycle workspaces.
    NextWorkspace,
    PrevWorkspace,
    /// Load and activate the named theme.
    SetTheme(String),
    /// Move keyboard focus to the named pane.
    FocusPane(String),
    /// Open the universal search overlay, optionally pre-filled.
    OpenSearch(String),
    /// Reload configuration from disk.
    ReloadConfig,
    /// Push a notification into the session.
    Notify(String),
    /// Clear the prompt pane's scrollback.
    ClearScrollback,
    /// Terminate the desktop session.
    Quit,
}

impl Effect {
    /// Human-readable effect name (used in tests and logs).
    pub fn kind(&self) -> &'static str {
        match self {
            Effect::Redraw => "redraw",
            Effect::SwitchWorkspace(_) => "switch-workspace",
            Effect::NextWorkspace => "next-workspace",
            Effect::PrevWorkspace => "prev-workspace",
            Effect::SetTheme(_) => "set-theme",
            Effect::FocusPane(_) => "focus-pane",
            Effect::OpenSearch(_) => "open-search",
            Effect::ReloadConfig => "reload-config",
            Effect::Notify(_) => "notify",
            Effect::ClearScrollback => "clear-scrollback",
            Effect::Quit => "quit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_stable() {
        assert_eq!(Effect::Quit.kind(), "quit");
        assert_eq!(Effect::SwitchWorkspace("x".into()).kind(), "switch-workspace");
    }
}
