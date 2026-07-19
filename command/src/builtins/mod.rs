//! The built-in command set.
//!
//! Commands are grouped by domain. Every group registers itself into the
//! [`Registry`]; adding a command means adding a type + one line here.

mod config_cmd;
mod dashboard_cmd;
mod meta;
mod notes_cmd;
mod plugin_cmd;
mod search_cmd;
mod system;
mod theme_cmd;
mod vision_cmd;
mod workspace_cmd;

use crate::Registry;

/// Register every built-in command.
pub fn register_all(r: &mut Registry) {
    meta::register(r);
    config_cmd::register(r);
    theme_cmd::register(r);
    workspace_cmd::register(r);
    notes_cmd::register(r);
    dashboard_cmd::register(r);
    search_cmd::register(r);
    plugin_cmd::register(r);
    vision_cmd::register(r);
    system::register(r);
}

#[cfg(test)]
mod tests {
    use crate::{Ctx, Registry};

    /// Every registered command must have non-empty metadata — the help
    /// system is only as good as its inputs.
    #[test]
    fn all_commands_have_docs() {
        let r = Registry::builtins();
        assert!(r.len() >= 40, "expected at least 40 commands, got {}", r.len());
        for name in r.names() {
            let cmd = r.find(name).unwrap();
            assert!(!cmd.about().is_empty(), "command '{name}' has no about text");
            assert_eq!(cmd.name(), name);
        }
    }

    /// A global smoke test across domains (each group has focused tests
    /// elsewhere, but wiring mistakes hide here).
    #[test]
    fn smoke_suite() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        for line in [
            "help theme",
            "echo hello",
            "version",
            "config get theme.name",
            "theme list",
            "workspace list",
            "notes new smoke-test",
            "notes add a quick thought",
            "notes list",
            "notes show smoke-test",
            "dashboard widgets",
            "plugin list",
            "vision providers",
            "calendar 7 2026",
            "uptime",
            "history",
            "search notes",
        ] {
            let out = r.exec(line, &mut ctx);
            assert!(out.is_ok(), "command failed: '{line}' → {:?}", out.err());
        }
        let out = r.exec("unknown-thing", &mut ctx);
        assert!(out.is_err());
    }
}
