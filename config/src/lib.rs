//! `slate-config` — configuration for Slate.
//!
//! * [`Document`] — an ordered TOML document (dependency-free parser in
//!   [`parser`]) with dotted-path `get`/`set`/`remove` and round-trip
//!   serialization
//! * [`Config`] — the typed configuration view: defaults → user file merge,
//!   `config get/set/reset`, XDG-aware paths
//!
//! The parser implements a pragmatic TOML subset (tables, keys, strings,
//! ints, floats, bools, single-line arrays) — enough for configuration, theme
//! files, and plugin manifests without pulling in `serde`/`toml`.
//! Unsupported constructs produce precise line-numbered errors.

#![forbid(unsafe_code)]

pub mod document;
pub mod parser;
pub mod paths;
pub mod value;

use std::path::PathBuf;

use slate_core::error::{Error, Result};

pub use document::Document;
pub use value::Value;

/// Embedded default configuration (a full TOML file, merged under the
/// user's `~/.config/slate/config.toml`).
pub const DEFAULT_CONFIG: &str = include_str!("../defaults/config.toml");

/// The merged, typed configuration.
#[derive(Clone, Debug)]
pub struct Config {
    pub doc: Document,
    /// Where the *user* config lives (set/reset write here).
    pub path: PathBuf,
}

impl Config {
    /// Defaults only (no user file) — used as fallback and for `--print-config`.
    pub fn defaults() -> Self {
        let doc = parser::parse(DEFAULT_CONFIG).unwrap_or_else(|_| Document::new());
        Config { doc, path: paths::config_file() }
    }

    /// Load defaults merged with the user config file (missing file is OK).
    pub fn load() -> Result<Self> {
        Self::load_from(paths::config_file())
    }

    pub fn load_from(path: PathBuf) -> Result<Self> {
        let mut base = parser::parse(DEFAULT_CONFIG)?;
        match std::fs::read_to_string(&path) {
            Ok(user_src) => {
                let user = parser::parse(&user_src)
                    .map_err(|e| Error::parse("config", format!("{}: {e}", path.display())))?;
                base = base.merged(user);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::from(e)),
        }
        Ok(Config { doc: base, path })
    }

    /// Serialize the effective configuration as TOML.
    pub fn to_toml(&self) -> String {
        self.doc.to_toml_string()
    }

    // ---- typed getters -------------------------------------------------

    pub fn get(&self, path: &str) -> Option<&Value> {
        self.doc.get(path)
    }

    pub fn get_str(&self, path: &str, default: &str) -> String {
        self.doc.get(path).and_then(Value::as_str).unwrap_or(default).to_string()
    }

    pub fn get_bool(&self, path: &str, default: bool) -> bool {
        self.doc.get(path).and_then(Value::as_bool).unwrap_or(default)
    }

    pub fn get_u16(&self, path: &str, default: u16) -> u16 {
        self.doc
            .get(path)
            .and_then(Value::as_int)
            .and_then(|v| u16::try_from(v).ok())
            .unwrap_or(default)
    }

    pub fn get_usize(&self, path: &str, default: usize) -> usize {
        self.doc
            .get(path)
            .and_then(Value::as_int)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(default)
    }

    pub fn get_string_list(&self, path: &str) -> Vec<String> {
        match self.doc.get(path) {
            Some(Value::Array(items)) => {
                items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
            }
            _ => Vec::new(),
        }
    }

    // ---- mutations ------------------------------------------------------

    /// `config set <path> <raw>` — parse `raw` leniently (bare words become
    /// strings) and persist.
    pub fn set_and_save(&mut self, path: &str, raw: &str) -> Result<()> {
        let value = parser::parse_fragment(raw);
        self.doc.set(path, value)?;
        self.save()
    }

    /// Restore the built-in defaults and write them to the user config.
    pub fn reset(&mut self) -> Result<()> {
        self.doc = parser::parse(DEFAULT_CONFIG)?;
        self.save()
    }

    /// Persist the current document to `self.path`.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, self.doc.to_toml_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses_and_exposes_values() {
        let cfg = Config::defaults();
        assert_eq!(cfg.get_str("theme.name", "?"), "slate-dark");
        assert!(cfg.get_bool("ui.status_bar", false));
        assert_eq!(cfg.get_u16("ui.tick_ms", 0), 250);
        assert!(cfg.get_string_list("dashboard.widgets").contains(&"cpu".to_string()));
        assert_eq!(cfg.get_str("vision.default_provider", "?"), "lens");
    }

    #[test]
    fn set_get_reset_roundtrip() {
        let dir = std::env::temp_dir().join(format!("slate-cfg-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let mut cfg = Config::load_from(path.clone()).unwrap();
        cfg.set_and_save("theme.name", "slate-light").unwrap();
        cfg.set_and_save("ui.tick_ms", "100").unwrap();
        assert_eq!(cfg.get_str("theme.name", "?"), "slate-light");

        // Reload from disk: values persist.
        let cfg2 = Config::load_from(path.clone()).unwrap();
        assert_eq!(cfg2.get_str("theme.name", "?"), "slate-light");
        assert_eq!(cfg2.get_u16("ui.tick_ms", 0), 100);

        // Reset restores defaults.
        let mut cfg3 = cfg2;
        cfg3.reset().unwrap();
        assert_eq!(cfg3.get_str("theme.name", "?"), "slate-dark");
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_config_overrides_and_merges() {
        let dir = std::env::temp_dir().join(format!("slate-cfg-merge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "[theme]\nname = \"contrast\"\n").unwrap();
        let cfg = Config::load_from(path).unwrap();
        assert_eq!(cfg.get_str("theme.name", "?"), "contrast");
        // default sections survive the merge
        assert_eq!(cfg.get_str("prompt.symbol", "?"), "❯");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
