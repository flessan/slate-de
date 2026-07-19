//! Colors, attributes and styled text.
//!
//! This is the single source of truth for styling across the workspace: the
//! theme engine produces [`Color`]s, the terminal engine turns them into SGR
//! sequences, and every pane renders [`Line`]s of [`Span`]s.

use std::fmt;
use std::ops::{BitOr, BitOrAssign};
use std::str::FromStr;

use crate::error::Error;
use crate::unicode;

/// A terminal color.
///
/// * `Default` — terminal's own foreground/background
/// * `Named(n)` — palette entry `0..=15` (dark 0-7, bright 8-15)
/// * `Indexed(n)` — xterm 256-color palette
/// * `Rgb` — true color; the backend degrades if unsupported
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    #[default]
    Default,
    Named(u8),
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    /// All names understood by the parser, for help text and validation.
    pub const NAMES: &'static [&'static str] = &[
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "gray", "grey",
        "bright-black", "bright-red", "bright-green", "bright-yellow", "bright-blue",
        "bright-magenta", "bright-cyan", "bright-white", "default",
    ];

    fn parse_named(name: &str) -> Option<Self> {
        let base = match name {
            "black" => 0,
            "red" => 1,
            "green" => 2,
            "yellow" => 3,
            "blue" => 4,
            "magenta" => 5,
            "cyan" => 6,
            "white" => 7,
            "gray" | "grey" | "bright-black" => 8,
            "bright-red" => 9,
            "bright-green" => 10,
            "bright-yellow" => 11,
            "bright-blue" => 12,
            "bright-magenta" => 13,
            "bright-cyan" => 14,
            "bright-white" => 15,
            _ => return None,
        };
        Some(Color::Named(base))
    }
}

impl FromStr for Color {
    type Err = Error;

    /// Parses `red`, `bright-blue`, `12` (indexed), `#rgb` and `#rrggbb`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("default") || s.is_empty() {
            return Ok(Color::Default);
        }
        let lower = s.to_ascii_lowercase();
        if let Some(c) = Color::parse_named(&lower) {
            return Ok(c);
        }
        if let Some(hex) = s.strip_prefix('#') {
            let err = || Error::invalid(format!("bad hex color '{s}'"));
            if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(err());
            }
            // All-ASCII input: byte slicing below is safe.
            let nib = |i: usize| u8::from_str_radix(&hex[i..=i], 16).map_err(|_| err());
            let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| err());
            return match hex.len() {
                3 => Ok(Color::Rgb(nib(0)? * 17, nib(1)? * 17, nib(2)? * 17)),
                6 => Ok(Color::Rgb(byte(0)?, byte(2)?, byte(4)?)),
                _ => Err(err()),
            };
        }
        if let Ok(n) = s.parse::<u8>() {
            return Ok(Color::Indexed(n));
        }
        Err(Error::invalid(format!("unknown color '{s}'")))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display output must round-trip through `FromStr`.
        const CANON: [&str; 16] = [
            "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
            "bright-black", "bright-red", "bright-green", "bright-yellow", "bright-blue",
            "bright-magenta", "bright-cyan", "bright-white",
        ];
        match self {
            Color::Default => write!(f, "default"),
            Color::Named(n) => write!(f, "{}", CANON[usize::from(*n.min(&15))]),
            Color::Indexed(n) => write!(f, "{n}"),
            Color::Rgb(r, g, b) => write!(f, "#{r:02x}{g:02x}{b:02x}"),
        }
    }
}

/// Bit set of text attributes (bold, italic, …).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Attrs(pub u8);

impl Attrs {
    pub const NONE: Attrs = Attrs(0);
    pub const BOLD: Attrs = Attrs(1 << 0);
    pub const DIM: Attrs = Attrs(1 << 1);
    pub const ITALIC: Attrs = Attrs(1 << 2);
    pub const UNDERLINE: Attrs = Attrs(1 << 3);
    pub const REVERSE: Attrs = Attrs(1 << 4);
    pub const CROSSED: Attrs = Attrs(1 << 5);

    pub fn contains(self, other: Attrs) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for Attrs {
    type Output = Attrs;
    fn bitor(self, rhs: Attrs) -> Attrs {
        Attrs(self.0 | rhs.0)
    }
}

impl BitOrAssign for Attrs {
    fn bitor_assign(&mut self, rhs: Attrs) {
        self.0 |= rhs.0;
    }
}

/// A complete style: foreground, background, attributes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub attrs: Attrs,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fg(mut self, c: Color) -> Self {
        self.fg = c;
        self
    }

    pub fn bg(mut self, c: Color) -> Self {
        self.bg = c;
        self
    }

    pub fn attrs(mut self, a: Attrs) -> Self {
        self.attrs = a;
        self
    }

    pub fn bold(mut self) -> Self {
        self.attrs |= Attrs::BOLD;
        self
    }

    pub fn dim(mut self) -> Self {
        self.attrs |= Attrs::DIM;
        self
    }
}

/// A styled piece of text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

impl Span {
    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Self { text: text.into(), style }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::styled(text, Style::default())
    }

    pub fn width(&self) -> usize {
        unicode::str_width(&self.text)
    }
}

/// A single line of styled text — what panes produce and widgets emit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub fn new(spans: Vec<Span>) -> Self {
        Self { spans }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self { spans: vec![Span::plain(text)] }
    }

    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Self { spans: vec![Span::styled(text, style)] }
    }

    pub fn push(&mut self, span: Span) {
        self.spans.push(span);
    }

    pub fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }

    /// Plain-text content with all styling dropped.
    pub fn to_plain(&self) -> String {
        let mut s = String::new();
        for sp in &self.spans {
            s.push_str(&sp.text);
        }
        s
    }

    pub fn is_empty(&self) -> bool {
        self.spans.iter().all(|s| s.text.is_empty())
    }
}

impl From<&str> for Line {
    fn from(s: &str) -> Self {
        Line::plain(s)
    }
}

impl From<String> for Line {
    fn from(s: String) -> Self {
        Line::plain(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_names_parse() {
        assert_eq!("red".parse::<Color>().unwrap(), Color::Named(1));
        assert_eq!("bright-blue".parse::<Color>().unwrap(), Color::Named(12));
        assert_eq!("grey".parse::<Color>().unwrap(), Color::Named(8));
        assert_eq!("default".parse::<Color>().unwrap(), Color::Default);
    }

    #[test]
    fn color_hex_parses() {
        assert_eq!("#ff0000".parse::<Color>().unwrap(), Color::Rgb(255, 0, 0));
        assert_eq!("#0f0".parse::<Color>().unwrap(), Color::Rgb(0, 255, 0));
        assert!("#zzzzzz".parse::<Color>().is_err());
        assert!("#12345".parse::<Color>().is_err());
    }

    #[test]
    fn color_indexes_parse() {
        assert_eq!("42".parse::<Color>().unwrap(), Color::Indexed(42));
        // 999 overflows u8 and is not a name either.
        assert!("999".parse::<Color>().is_err());
    }

    #[test]
    fn line_width_sums_spans() {
        let l = Line::new(vec![Span::plain("ab"), Span::plain("界")]);
        assert_eq!(l.width(), 4); // 1 + 1 + 2 (wide char)
        assert_eq!(l.to_plain(), "ab界");
    }

    #[test]
    fn attrs_bitops() {
        let a = Attrs::BOLD | Attrs::DIM;
        assert!(a.contains(Attrs::BOLD));
        assert!(a.contains(Attrs::DIM));
        assert!(!a.contains(Attrs::ITALIC));
    }
}
