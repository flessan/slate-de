//! [`Ctx`] — the command context: the complete, inspectable state of the
//! desktop. Commands get `&mut Ctx`; nothing else in the process owns
//! desktop state, so commands are trivially testable headlessly.

use std::collections::VecDeque;
use std::path::PathBuf;

use slate_config::{paths, Config, Value};
use slate_core::effect::Effect;
use slate_core::error::Result;
use slate_core::layout::LayoutSpec;
use slate_core::notify::{Level, Notification};
use slate_core::workspace::Workspaces;
use slate_launcher::{Index, Item, ItemKind};
use slate_notes::Notebook;
use slate_plugin::PluginRegistry;
use slate_theme::ThemeManager;
use slate_vision::VisionRegistry;

use crate::Registry;

const MAX_NOTIFICATIONS: usize = 100;

/// The desktop's addressable state.
pub struct Ctx {
    pub cfg: Config,
    pub themes: ThemeManager,
    pub notes: Notebook,
    pub plugins: PluginRegistry,
    pub vision: VisionRegistry,
    pub workspaces: Workspaces,
    /// Universal search index (rebuilt by [`Ctx::refresh_search`]).
    pub index: Index,
    pub history: Vec<String>,
    pub notifications: VecDeque<Notification>,
    /// Prompt working directory (`cd`, `git` widget, `run`).
    pub cwd: PathBuf,
    /// When true, effects are not executed (no UI to drive).
    pub headless: bool,
    /// Deterministic demo mode (snapshots/tests).
    pub demo: bool,
    /// Dirty flags the shell consumes to rebuild derived state.
    pub search_dirty: bool,
    pub dashboard_dirty: bool,
    pub layout_dirty: bool,
    next_id: u64,
}

impl Ctx {
    /// Interactive startup.
    pub fn load() -> Result<Self> {
        Self::load_inner(false)
    }

    /// Headless startup (`slate run`, integration tests).
    pub fn load_headless() -> Result<Self> {
        Self::load_inner(true)
    }

    /// A context with no user state at all: temp dirs, defaults, headless.
    /// Used across the test suite (never touches the real config).
    pub fn ephemeral() -> Self {
        let root = std::env::temp_dir().join(format!(
            "slate-ephemeral-{}-{}",
            std::process::id(),
            slate_core::civil::DateTime::now_utc().secs
        ));
        let mut ctx = Ctx {
            cfg: Config::defaults(),
            themes: ThemeManager::builtins(),
            notes: Notebook::new(root.join("notes")),
            plugins: PluginRegistry::empty(root.join("plugins")),
            vision: VisionRegistry::with_default("lens"),
            workspaces: Workspaces::builtins(),
            index: Index::default(),
            history: Vec::new(),
            notifications: VecDeque::new(),
            cwd: std::env::temp_dir(),
            headless: true,
            demo: false,
            search_dirty: true,
            dashboard_dirty: false,
            layout_dirty: false,
            next_id: 1,
        };
        ctx.notes.init().ok();
        ctx
    }

    fn load_inner(headless: bool) -> Result<Self> {
        paths::ensure_dirs()?;
        let mut warnings: Vec<String> = Vec::new();
        let cfg = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                warnings.push(format!("config error, using defaults — {e}"));
                Config::defaults()
            }
        };
        let mut themes = ThemeManager::builtins();
        let (_n, errs) = themes.load_user_themes(&paths::themes_dir());
        warnings.extend(errs);

        let notes_dir = configured_dir(&cfg, "notes.dir", paths::notes_dir);
        let plugins_dir = configured_dir(&cfg, "plugins.dir", paths::plugins_dir);

        let mut ctx = Ctx {
            vision: VisionRegistry::with_default(cfg.get_str("vision.default_provider", "lens")),
            notes: Notebook::new(notes_dir),
            plugins: PluginRegistry::load(plugins_dir),
            workspaces: Workspaces::builtins(),
            index: Index::default(),
            history: Vec::new(),
            notifications: VecDeque::new(),
            cwd: std::env::current_dir().unwrap_or_else(|_| paths::home_dir()),
            headless,
            demo: false,
            search_dirty: true,
            dashboard_dirty: false,
            layout_dirty: false,
            next_id: 1,
            cfg,
            themes,
        };
        ctx.notes.init().ok();
        if let Err(e) = paths::ensure_dirs() {
            warnings.push(format!("could not create data dirs: {e}"));
        }
        ctx.apply_config_layouts();
        ctx.apply_theme();
        ctx.load_history();
        for w in warnings {
            ctx.notify(Level::Warn, w);
        }
        Ok(ctx)
    }

    // ---- configuration helpers --------------------------------------------

    pub fn theme(&self) -> &slate_theme::Theme {
        self.themes.current()
    }

    /// Activate a theme (and its layout overrides); persists the choice.
    pub fn set_theme(&mut self, name: &str) -> Result<()> {
        if !self.themes.set_current(name) {
            return Err(slate_core::Error::not_found(format!("theme '{name}'")));
        }
        self.cfg.set_and_save("theme.name", name)?;
        self.apply_theme();
        Ok(())
    }

    fn apply_theme(&mut self) {
        // Theme layout overrides land on top of config layouts.
        for (ws, spec) in self.theme().layout_overrides.clone() {
            if let Ok(layout) = LayoutSpec::parse(&spec) {
                self.workspaces.set_layout(&ws, layout);
                self.layout_dirty = true;
            }
        }
    }

    fn apply_config_layouts(&mut self) {
        if let Some(Value::Table(table)) = self.cfg.get("workspace_layouts") {
            for (name, v) in table {
                if let Some(spec) = v.as_str() {
                    match LayoutSpec::parse(spec) {
                        Ok(layout) => {
                            self.workspaces.set_layout(name, layout);
                            self.layout_dirty = true;
                        }
                        Err(e) => {
                            self.notify(Level::Warn, format!("workspace_layouts.{name}: {e}"));
                        }
                    }
                }
            }
        }
        let default_ws = self.cfg.get_str("workspaces.default", "main");
        if self.workspaces.contains(&default_ws) {
            self.workspaces.switch(&default_ws);
        }
    }

    /// Reload config from disk (the `reload` builtin / IPC).
    pub fn reload(&mut self) -> Result<()> {
        self.cfg = Config::load()?;
        self.apply_config_layouts();
        let theme = self.cfg.get_str("theme.name", "slate-dark");
        if self.themes.set_current(&theme) {
            self.apply_theme();
        }
        self.vision.set_default(&self.cfg.get_str("vision.default_provider", "lens"));
        self.layout_dirty = true;
        self.dashboard_dirty = true;
        self.search_dirty = true;
        Ok(())
    }

    pub fn weather_location(&self) -> Option<String> {
        let loc = self.cfg.get_str("dashboard.weather_location", "");
        if loc.is_empty() {
            None
        } else {
            Some(loc)
        }
    }

    pub fn dashboard_widget_names(&self) -> Vec<String> {
        self.cfg.get_string_list("dashboard.widgets")
    }

    // ---- notifications ------------------------------------------------------

    pub fn notify(&mut self, level: Level, text: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        if self.notifications.len() >= MAX_NOTIFICATIONS {
            self.notifications.pop_front();
        }
        self.notifications.push_back(Notification::new(id, level, text));
        id
    }

    pub fn clear_notifications(&mut self) {
        self.notifications.clear();
    }

    // ---- history --------------------------------------------------------------

    pub fn history_push(&mut self, line: &str) {
        if self.history.last().map(|l| l.as_str()) == Some(line) {
            return; // no consecutive duplicates
        }
        let cap = self.cfg.get_usize("prompt.history_size", 1000);
        self.history.push(line.to_string());
        if self.history.len() > cap {
            self.history.drain(..self.history.len() - cap);
        }
        if let Err(_e) = append_history_line(line) { /* history is best-effort */ }
        self.search_dirty = true;
    }

    fn load_history(&mut self) {
        let cap = self.cfg.get_usize("prompt.history_size", 1000);
        if let Ok(text) = std::fs::read_to_string(paths::history_file()) {
            let lines: Vec<String> = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect();
            let start = lines.len().saturating_sub(cap);
            self.history = lines[start..].to_vec();
        }
    }

    // ---- universal search ------------------------------------------------------

    /// Rebuild the search index from all sources (cheap; ~ms for a few
    /// thousand items — see `tests/perf.rs`).
    pub fn refresh_search(&mut self, registry: &Registry) {
        let mut items: Vec<Item> = Vec::new();
        // 1) commands (highest boost — they're the OS vocabulary)
        for name in registry.names() {
            if let Some(cmd) = registry.find(name) {
                items.push(
                    Item::new(ItemKind::Command, cmd.name(), cmd.about(), cmd.name()).with_boost(50),
                );
            }
        }
        // 2) notes
        if let Ok(notes) = self.notes.list() {
            for n in notes {
                let action = format!("notes show {}", n.slug);
                items.push(Item::new(ItemKind::Note, n.title, n.preview(), action).with_boost(10));
            }
        }
        // 3) workspaces / themes
        for name in self.workspaces.names() {
            let action = format!("workspace switch {name}");
            items.push(Item::new(ItemKind::Workspace, name, "workspace", action).with_boost(10));
        }
        for name in self.themes.names() {
            let action = format!("theme set {name}");
            items.push(Item::new(ItemKind::Theme, name, "theme", action).with_boost(10));
        }
        // 4) plugins
        for p in self.plugins.list() {
            let action = format!("plugin run {}", p.manifest.name);
            items.push(
                Item::new(ItemKind::Plugin, p.manifest.name.clone(), p.manifest.description.clone(), action)
                    .with_boost(10),
            );
        }
        // 5) settings keys
        for key in self.cfg.doc.keys() {
            let action = format!("config get {key}");
            items.push(Item::new(ItemKind::Setting, key, "setting", action));
        }
        // 6) history (most recent last; reversed so newest ranks by boost)
        for (i, line) in self.history.iter().rev().enumerate().take(200) {
            items.push(
                Item::new(ItemKind::History, line.clone(), "history", line.clone())
                    .with_boost(-(i as i64)),
            );
        }
        // 7) bookmarks
        if let Some(Value::Table(bms)) = self.cfg.get("bookmarks") {
            for (name, v) in bms {
                if let Some(url) = v.as_str() {
                    let action = format!("open {url}");
                    items.push(Item::new(ItemKind::Bookmark, name.clone(), url, action));
                }
            }
        }
        // 8) files (bounded walk)
        let roots = self.cfg.get_string_list("search.roots");
        let depth = self.cfg.get_usize("search.depth", 3);
        let max = self.cfg.get_usize("search.max_entries", 4000);
        let hidden = self.cfg.get_bool("search.include_hidden", false);
        for path in collect_files(&roots, depth, max, hidden) {
            let title = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let detail = path.to_string_lossy().into_owned();
            let action = format!("open {detail}");
            items.push(Item::new(ItemKind::File, title, detail, action));
        }
        self.index = Index::new(items);
        self.search_dirty = false;
    }

    /// Convenience: notify + effect pair for a command that wants to surface
    /// a message both in scrollback and in the notification center.
    pub fn notify_effect(&mut self, level: Level, text: &str) -> Effect {
        self.notify(level, text.to_string());
        Effect::Notify(text.to_string())
    }
}

fn configured_dir(cfg: &Config, key: &str, default: fn() -> PathBuf) -> PathBuf {
    let v = cfg.get_str(key, "");
    if v.is_empty() {
        default()
    } else {
        PathBuf::from(expand_tilde(&v))
    }
}

/// Expand a leading `~` to the home directory.
pub fn expand_tilde(p: &str) -> String {
    if p == "~" {
        return paths::home_dir().to_string_lossy().into_owned();
    }
    if let Some(rest) = p.strip_prefix("~/") {
        return paths::home_dir().join(rest).to_string_lossy().into_owned();
    }
    p.to_string()
}

fn append_history_line(line: &str) -> std::io::Result<()> {
    let file = paths::history_file();
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file)?;
    writeln!(f, "{line}")
}

/// Bounded recursive file collection for the search index.
pub fn collect_files(roots: &[String], depth: usize, max: usize, include_hidden: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        let root = PathBuf::from(expand_tilde(root));
        walk(&root, depth, max, include_hidden, &mut out);
        if out.len() >= max {
            break;
        }
    }
    out
}

fn walk(dir: &PathBuf, depth: usize, max: usize, hidden: bool, out: &mut Vec<PathBuf>) {
    if out.len() >= max {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        if out.len() >= max {
            return;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !hidden && name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                walk(&path, depth - 1, max, hidden, out);
            }
        } else {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ephemeral_ctx_is_sane() {
        let mut ctx = Ctx::ephemeral();
        assert!(ctx.headless);
        assert!(ctx.notes.list().unwrap().is_empty());
        assert!(ctx.themes.set_current("slate-light"));
        let id = ctx.notify(Level::Info, "hello");
        assert_eq!(id, 1);
        assert_eq!(ctx.notifications[0].text, "hello");
        cmd_exec_smoke(&mut ctx);
    }

    fn cmd_exec_smoke(ctx: &mut Ctx) {
        let reg = Registry::builtins();
        ctx.refresh_search(&reg);
        assert!(!ctx.index.is_empty());
        // Every command is searchable.
        let hits = ctx.index.query("notes", 5);
        assert!(hits.iter().any(|i| i.title == "notes"));
    }

    #[test]
    fn tilde_expansion() {
        assert!(expand_tilde("~/x").contains("/x"));
        assert_eq!(expand_tilde("/abs"), "/abs");
        assert!(!expand_tilde("~").ends_with('/'));
    }

    #[test]
    fn history_dedups_consecutive() {
        let mut ctx = Ctx::ephemeral();
        ctx.history_push("a");
        ctx.history_push("a");
        ctx.history_push("b");
        assert_eq!(ctx.history.len(), 2);
    }
}
