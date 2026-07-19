//! XDG-aware filesystem locations.
//!
//! Resolution order (highest priority first):
//! 1. `SLATE_*_DIR` environment overrides (handy for tests, distros, portable
//!    installs)
//! 2. XDG base directories (`XDG_CONFIG_HOME`, …)
//! 3. The classic `~/.config`, `~/.local/share`, `~/.local/state` fallbacks

use std::path::PathBuf;

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The user's home directory (`.` if unknown).
pub fn home_dir() -> PathBuf {
    env_path("HOME").unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_dir() -> PathBuf {
    env_path("SLATE_CONFIG_DIR")
        .or_else(|| env_path("XDG_CONFIG_HOME").map(|p| p.join("slate")))
        .unwrap_or_else(|| home_dir().join(".config").join("slate"))
}

pub fn data_dir() -> PathBuf {
    env_path("SLATE_DATA_DIR")
        .or_else(|| env_path("XDG_DATA_HOME").map(|p| p.join("slate")))
        .unwrap_or_else(|| home_dir().join(".local").join("share").join("slate"))
}

pub fn state_dir() -> PathBuf {
    env_path("SLATE_STATE_DIR")
        .or_else(|| env_path("XDG_STATE_HOME").map(|p| p.join("slate")))
        .unwrap_or_else(|| home_dir().join(".local").join("state").join("slate"))
}

pub fn runtime_dir() -> PathBuf {
    env_path("SLATE_RUNTIME_DIR")
        .or_else(|| env_path("XDG_RUNTIME_DIR"))
        .unwrap_or_else(std::env::temp_dir)
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn notes_dir() -> PathBuf {
    data_dir().join("notes")
}

pub fn plugins_dir() -> PathBuf {
    data_dir().join("plugins")
}

pub fn themes_dir() -> PathBuf {
    config_dir().join("themes")
}

pub fn history_file() -> PathBuf {
    state_dir().join("history")
}

pub fn log_file() -> PathBuf {
    state_dir().join("slate.log")
}

pub fn socket_file() -> PathBuf {
    runtime_dir().join("slate.sock")
}

/// Create every directory Slate needs (idempotent).
pub fn ensure_dirs() -> std::io::Result<()> {
    for dir in [config_dir(), data_dir(), state_dir(), notes_dir(), plugins_dir(), themes_dir()] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_absolute_and_namespaced() {
        std::env::remove_var("SLATE_CONFIG_DIR");
        let cfg = config_dir();
        assert!(cfg.ends_with("slate"));
        assert!(config_file().ends_with("slate/config.toml"));
        assert!(notes_dir().ends_with("slate/notes"));
        assert!(socket_file().ends_with("slate.sock"));
    }

    #[test]
    fn env_override_wins() {
        std::env::set_var("SLATE_CONFIG_DIR", "/tmp/slate-test-config");
        assert_eq!(config_dir(), PathBuf::from("/tmp/slate-test-config"));
        assert_eq!(config_file(), PathBuf::from("/tmp/slate-test-config/config.toml"));
        std::env::remove_var("SLATE_CONFIG_DIR");
    }
}
