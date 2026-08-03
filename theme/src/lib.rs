//! `slate-theme` — Slate's theme engine.
//!
//! A theme is more than colors (see `docs/THEMES.md`):
//!
//! * a **palette** (semantic colors: `accent`, `muted`, `ok`, …)
//! * a **geometry** choice (borders: rounded/plain/double/ASCII)
//! * a **glyph set** with automatic ASCII fallback (`unicode = false`)
//! * optional **per-workspace layout overrides**
//!
//! Themes are TOML documents parsed by `slate-config`'s engine. Built-ins
//! ship inside the binary; users add more in `~/.config/slate/themes/`.

#![forbid(unsafe_code)]

use std::path::Path;

use slate_config::{parser, Document, Value};
use slate_core::error::Result;
use slate_core::style::{Attrs, Color, Style};
use slate_term::widgets::BorderKind;

/// The four (plus ASCII) bundled themes.
pub const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("slate-dark", include_str!("../themes/slate-dark.toml")),
    ("slate-light", include_str!("../themes/slate-light.toml")),
    ("contrast", include_str!("../themes/contrast.toml")),
    ("mono", include_str!("../themes/mono.toml")),
    ("ascii", include_str!("../themes/ascii.toml")),
];

/// Semantic color roles. Everything in the UI goes through these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent2: Color,
    pub ok: Color,
    pub warn: Color,
    pub error: Color,
    pub selection: Color,
    pub border: Color,
    pub border_focused: Color,
    pub bar_bg: Color,
    pub bar_fg: Color,
}

impl Palette {
    /// The slate-dark defaults, used as fallback for partial theme files.
    pub fn slate_dark() -> Self {
        Palette {
            bg: Color::Rgb(16, 16, 22),
            fg: Color::Rgb(228, 228, 236),
            muted: Color::Rgb(132, 132, 148),
            accent: Color::Rgb(122, 162, 247),
            accent2: Color::Rgb(187, 154, 247),
            ok: Color::Rgb(158, 206, 106),
            warn: Color::Rgb(224, 175, 104),
            error: Color::Rgb(247, 118, 141),
            selection: Color::Rgb(42, 50, 78),
            border: Color::Rgb(56, 60, 82),
            border_focused: Color::Rgb(122, 162, 247),
            bar_bg: Color::Rgb(24, 24, 34),
            bar_fg: Color::Rgb(228, 228, 236),
        }
    }
}

/// `(name, unicode, ascii)` triples for the built-in glyph table.
const GLYPHS: &[(&str, &str, &str)] = &[
    ("prompt", "❯", ">"),
    ("search", "⌕", "/"),
    ("note", "✎", "n"),
    ("ok", "✓", "+"),
    ("err", "✗", "x"),
    ("warn", "⚠", "!"),
    ("bell", "♪", "*"),
    ("pin", "◈", "p"),
    ("dot", "•", "."),
    ("tri", "▸", ">"),
    ("music", "♫", "m"),
    ("net", "⇅", "n"),
    ("up", "↑", "u"),
    ("dn", "↓", "d"),
    ("gauge_full", "█", "#"),
    ("gauge_empty", "░", "-"),
];

/// A complete theme.
#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub palette: Palette,
    pub unicode: bool,
    pub borders: BorderKind,
    glyph_overrides: Vec<(String, String)>,
    /// `workspace → layout spec` overrides from the `[layout]` section.
    pub layout_overrides: Vec<(String, String)>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            name: "slate-dark".into(),
            palette: Palette::slate_dark(),
            unicode: true,
            borders: BorderKind::Rounded,
            glyph_overrides: Vec::new(),
            layout_overrides: Vec::new(),
        }
    }
}

impl Theme {
    /// Resolve a named glyph, honoring `unicode = false`.
    pub fn glyph(&self, key: &str) -> &str {
        for (k, v) in &self.glyph_overrides {
            if k == key {
                return v;
            }
        }
        for (k, uni, ascii) in GLYPHS {
            if *k == key {
                return if self.unicode { uni } else { ascii };
            }
        }
        "?"
    }

    // ---- convenient style builders (used everywhere in the UI) ----------

    /// Body text.
    pub fn text(&self) -> Style {
        Style::new().fg(self.palette.fg).bg(self.palette.bg)
    }

    pub fn dim(&self) -> Style {
        Style::new().fg(self.palette.muted).bg(self.palette.bg)
    }

    pub fn accent(&self) -> Style {
        Style::new().fg(self.palette.accent).bg(self.palette.bg).attrs(Attrs::BOLD)
    }

    pub fn accent2(&self) -> Style {
        Style::new().fg(self.palette.accent2).bg(self.palette.bg)
    }

    pub fn ok(&self) -> Style {
        Style::new().fg(self.palette.ok).bg(self.palette.bg)
    }

    pub fn warn(&self) -> Style {
        Style::new().fg(self.palette.warn).bg(self.palette.bg)
    }

    pub fn error(&self) -> Style {
        Style::new().fg(self.palette.error).bg(self.palette.bg)
    }

    /// Focused pane border / title.
    pub fn border_focused(&self) -> Style {
        Style::new().fg(self.palette.border_focused).bg(self.palette.bg)
    }

    pub fn border(&self) -> Style {
        Style::new().fg(self.palette.border).bg(self.palette.bg)
    }

    /// Selected row highlight.
    pub fn selection(&self) -> Style {
        Style::new().fg(self.palette.fg).bg(self.palette.selection).attrs(Attrs::BOLD)
    }

    /// Status bar.
    pub fn bar(&self) -> Style {
        Style::new().fg(self.palette.bar_fg).bg(self.palette.bar_bg)
    }

    pub fn bar_accent(&self) -> Style {
        Style::new().fg(self.palette.accent).bg(self.palette.bar_bg).attrs(Attrs::BOLD)
    }

    pub fn gauge_ratio_color(&self, ratio: f64) -> Color {
        if ratio >= 0.9 {
            self.palette.error
        } else if ratio >= 0.7 {
            self.palette.warn
        } else {
            self.palette.ok
        }
    }

    /// Build from a theme document (missing values fall back to slate-dark).
    pub fn from_document(doc: &Document) -> Theme {
        let mut theme = Theme::default();
        if let Some(name) = doc.get("name").and_then(Value::as_str) {
            theme.name = name.to_string();
        }
        let mut pal = Palette::slate_dark();
        let set = |target: &mut Color, key: &str| {
            if let Some(c) = doc.get(&format!("palette.{key}")).and_then(Value::as_str) {
                if let Ok(c) = c.parse::<Color>() {
                    *target = c;
                }
            }
        };
        set(&mut pal.bg, "bg");
        set(&mut pal.fg, "fg");
        set(&mut pal.muted, "muted");
        set(&mut pal.accent, "accent");
        set(&mut pal.accent2, "accent2");
        set(&mut pal.ok, "ok");
        set(&mut pal.warn, "warn");
        set(&mut pal.error, "error");
        set(&mut pal.selection, "selection");
        set(&mut pal.border, "border");
        set(&mut pal.border_focused, "border_focused");
        set(&mut pal.bar_bg, "bar_bg");
        set(&mut pal.bar_fg, "bar_fg");
        theme.palette = pal;

        if let Some(u) = doc.get("style.unicode").and_then(Value::as_bool) {
            theme.unicode = u;
        }
        if let Some(b) = doc.get("style.borders").and_then(Value::as_str) {
            theme.borders = match b {
                "plain" => BorderKind::Plain,
                "double" => BorderKind::Double,
                "ascii" => BorderKind::Ascii,
                _ => BorderKind::Rounded,
            };
        }
        if !theme.unicode {
            theme.borders = BorderKind::Ascii;
        }
        if let Some(Value::Table(glyphs)) = doc.get("glyphs") {
            for (k, v) in glyphs {
                if let Some(s) = v.as_str() {
                    theme.glyph_overrides.push((k.clone(), s.to_string()));
                }
            }
        }
        if let Some(Value::Table(layouts)) = doc.get("layout") {
            for (k, v) in layouts {
                if let Some(s) = v.as_str() {
                    theme.layout_overrides.push((k.clone(), s.to_string()));
                }
            }
        }
        theme
    }

    pub fn from_toml(src: &str) -> Result<Theme> {
        Ok(Theme::from_document(&Document::from_value(parser::parse(src)?)))
    }

    pub fn to_document(&self) -> Document {
        let mut doc = Document::new();
        let _ = doc.set("name", Value::Str(self.name.clone()));
        let _ = doc.set("palette.bg", Value::Str(self.palette.bg.to_string()));
        let _ = doc.set("palette.fg", Value::Str(self.palette.fg.to_string()));
        let _ = doc.set("palette.muted", Value::Str(self.palette.muted.to_string()));
        let _ = doc.set("palette.accent", Value::Str(self.palette.accent.to_string()));
        let _ = doc.set("palette.accent2", Value::Str(self.palette.accent2.to_string()));
        let _ = doc.set("palette.ok", Value::Str(self.palette.ok.to_string()));
        let _ = doc.set("palette.warn", Value::Str(self.palette.warn.to_string()));
        let _ = doc.set("palette.error", Value::Str(self.palette.error.to_string()));
        let _ = doc.set("palette.selection", Value::Str(self.palette.selection.to_string()));
        let _ = doc.set("palette.border", Value::Str(self.palette.border.to_string()));
        let _ = doc.set(
            "palette.border_focused",
            Value::Str(self.palette.border_focused.to_string()),
        );
        let _ = doc.set("palette.bar_bg", Value::Str(self.palette.bar_bg.to_string()));
        let _ = doc.set("palette.bar_fg", Value::Str(self.palette.bar_fg.to_string()));
        let _ = doc.set(
            "style.borders",
            Value::Str(
                match self.borders {
                    BorderKind::Rounded => "rounded",
                    BorderKind::Plain => "plain",
                    BorderKind::Double => "double",
                    BorderKind::Ascii => "ascii",
                }
                .to_string(),
            ),
        );
        let _ = doc.set("style.unicode", Value::Bool(self.unicode));
        for (k, v) in &self.glyph_overrides {
            let _ = doc.set(&format!("glyphs.{k}"), Value::Str(v.clone()));
        }
        for (k, v) in &self.layout_overrides {
            let _ = doc.set(&format!("layout.{k}"), Value::Str(v.clone()));
        }
        doc
    }
}

/// All known themes plus the active selection.
pub struct ThemeManager {
    themes: Vec<Theme>,
    current: usize,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::builtins()
    }
}

impl ThemeManager {
    pub fn builtins() -> Self {
        let themes = BUILTIN_THEMES
            .iter()
            .map(|(name, src)| Theme::from_toml(src).unwrap_or_else(|_| Theme {
                name: (*name).to_string(),
                ..Theme::default()
            }))
            .collect();
        ThemeManager { themes, current: 0 }
    }

    /// Load user themes from `~/.config/slate/themes/*.toml`. Returns
    /// `(loaded, errors)`.
    pub fn load_user_themes(&mut self, dir: &Path) -> (usize, Vec<String>) {
        let mut loaded = 0;
        let mut errors = Vec::new();
        let Ok(rd) = std::fs::read_dir(dir) else { return (0, errors) };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            match std::fs::read_to_string(&path).map_err(slate_core::Error::from).and_then(|src| Theme::from_toml(&src)) {
                Ok(theme) => {
                    if let Some(pos) = self.themes.iter().position(|t| t.name == theme.name) {
                        self.themes[pos] = theme;
                    } else {
                        self.themes.push(theme);
                    }
                    loaded += 1;
                }
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        (loaded, errors)
    }

    pub fn names(&self) -> Vec<&str> {
        self.themes.iter().map(|t| t.name.as_str()).collect()
    }

    pub fn current(&self) -> &Theme {
        &self.themes[self.current]
    }

    pub fn current_name(&self) -> &str {
        &self.themes[self.current].name
    }

    pub fn set_current(&mut self, name: &str) -> bool {
        if let Some(i) = self.themes.iter().position(|t| t.name == name) {
            self.current = i;
            true
        } else {
            false
        }
    }

    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.themes.iter().find(|t| t.name == name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Theme> {
        self.themes.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_load() {
        let tm = ThemeManager::builtins();
        assert!(tm.names().contains(&"slate-dark"));
        assert!(tm.names().contains(&"slate-light"));
        assert!(tm.names().contains(&"ascii"));
        assert!(tm.names().contains(&"contrast"));
        assert!(tm.names().contains(&"mono"));
        assert_eq!(tm.current_name(), "slate-dark");
    }

    #[test]
    fn ascii_theme_forces_ascii() {
        let tm = ThemeManager::builtins();
        let ascii = tm.get("ascii").unwrap();
        assert!(!ascii.unicode);
        assert_eq!(ascii.borders, BorderKind::Ascii);
        assert_eq!(ascii.glyph("prompt"), ">");
        assert_eq!(ascii.glyph("ok"), "+");
        let dark = tm.get("slate-dark").unwrap();
        assert_eq!(dark.glyph("prompt"), "❯");
    }

    #[test]
    fn switching() {
        let mut tm = ThemeManager::builtins();
        assert!(tm.set_current("slate-light"));
        assert_eq!(tm.current_name(), "slate-light");
        assert!(!tm.set_current("nope"));
        assert_eq!(tm.current_name(), "slate-light");
    }

    #[test]
    fn ser_de_roundtrip() {
        let tm = ThemeManager::builtins();
        let dark = tm.get("slate-dark").unwrap();
        let doc = dark.to_document();
        let back = Theme::from_document(&doc);
        assert_eq!(back.name, dark.name);
        assert_eq!(back.palette, dark.palette);
        assert_eq!(back.borders, dark.borders);
        assert_eq!(back.unicode, dark.unicode);
    }

        #[test]
    fn user_theme_overrides() {
        let src = r#"
name = "mine"
[palette]
accent = "#ff0000"
[style]
borders = "double"
[glyphs]
prompt = ">"
"#;      
        let t = Theme::from_toml(src).unwrap();
        assert_eq!(t.palette.accent, Color::Rgb(255, 0, 0));
        assert_eq!(t.palette.fg, Palette::slate_dark().fg); // fallback
        assert_eq!(t.borders, BorderKind::Double);
        assert_eq!(t.glyph("prompt"), ">");
    }

    #[test]
    fn styles_build() {
        let t = Theme::default();
        assert_eq!(t.text().fg, t.palette.fg);
        assert!(t.accent().attrs.contains(Attrs::BOLD));
        assert_eq!(t.gauge_ratio_color(0.95), t.palette.error);
        assert_eq!(t.gauge_ratio_color(0.75), t.palette.warn);
        assert_eq!(t.gauge_ratio_color(0.2), t.palette.ok);
    }
}
