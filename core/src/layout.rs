//! Declarative pane layouts.
//!
//! Slate's desktop layout is defined by a tiny, human-writable DSL:
//!
//! ```text
//! expr  := row ("/" row)*        # "/" stacks rows vertically
//! row   := term ("|" term)*      # "|" places columns side by side
//! term  := IDENT [ ":" weight ] | "(" expr ")" [ ":" weight ]
//! ```
//!
//! * weights are relative (e.g. `prompt:72 | notes:28` ≈ 72 % / 28 %)
//! * panes are addressed by name (`prompt`, `notes`, `dashboard`, `logs`, …)
//!
//! Example for the default desktop:
//!
//! ```text
//! (prompt:72 | (notes:58 / dashboard:42)):80 / logs:20
//! ```

use crate::error::{Error, Result};
use crate::rect::{split_rects, Direction, Rect};

/// A parsed layout tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutSpec {
    /// A leaf pane addressed by name.
    Panel { name: String, weight: u16 },
    /// A split of `children` along `dir`.
    Split { dir: Direction, weight: u16, children: Vec<LayoutSpec> },
}

impl LayoutSpec {
    /// A single pane filling its region.
    pub fn panel(name: impl Into<String>) -> Self {
        LayoutSpec::Panel { name: name.into(), weight: 1 }
    }

    /// Weight of this node within its parent split.
    pub fn weight(&self) -> u16 {
        match self {
            LayoutSpec::Panel { weight, .. } | LayoutSpec::Split { weight, .. } => *weight,
        }
    }

    /// Names of all leaf panes, in depth-first order.
    pub fn names(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.collect_names(&mut out);
        out
    }

    fn collect_names<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            LayoutSpec::Panel { name, .. } => out.push(name.as_str()),
            LayoutSpec::Split { children, .. } => {
                for c in children {
                    c.collect_names(out);
                }
            }
        }
    }

    /// Resolve the tree into concrete rectangles for `area`.
    pub fn compute(&self, area: Rect, gap: u16) -> Vec<(String, Rect)> {
        let mut out = Vec::new();
        self.walk(area, gap, &mut out);
        out
    }

    fn walk(&self, area: Rect, gap: u16, out: &mut Vec<(String, Rect)>) {
        match self {
            LayoutSpec::Panel { name, .. } => out.push((name.clone(), area)),
            LayoutSpec::Split { dir, children, .. } => {
                let weights: Vec<u16> = children.iter().map(LayoutSpec::weight).collect();
                let rects = split_rects(area, *dir, &weights, gap);
                for (child, rect) in children.iter().zip(rects) {
                    child.walk(rect, gap, out);
                }
            }
        }
    }

    /// Parse a layout from the DSL.
    pub fn parse(src: &str) -> Result<LayoutSpec> {
        let mut p = Parser { bytes: src.as_bytes(), pos: 0 };
        let spec = p.expr()?;
        p.skip_ws();
        if p.pos != p.bytes.len() {
            return Err(p.err(format!("unexpected '{}'", p.bytes[p.pos] as char)));
        }
        Ok(spec)
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, msg: impl Into<String>) -> Error {
        Error::parse("layout", format!("{} (column {})", msg.into(), self.pos + 1))
    }

    fn skip_ws(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\n')) {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.bytes.get(self.pos).copied()
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// expr := row ("/" row)*
    fn expr(&mut self) -> Result<LayoutSpec> {
        let first = self.row()?;
        let mut children = vec![first];
        while self.eat(b'/') {
            children.push(self.row()?);
        }
        if children.len() == 1 {
            return Ok(children.pop().unwrap_or_else(|| LayoutSpec::panel("prompt")));
        }
        Ok(LayoutSpec::Split { dir: Direction::Vertical, weight: 1, children })
    }

    /// row := term ("|" term)*
    fn row(&mut self) -> Result<LayoutSpec> {
        let first = self.term()?;
        let mut children = vec![first];
        while self.eat(b'|') {
            children.push(self.term()?);
        }
        if children.len() == 1 {
            return Ok(children.pop().unwrap_or_else(|| LayoutSpec::panel("prompt")));
        }
        Ok(LayoutSpec::Split { dir: Direction::Horizontal, weight: 1, children })
    }

    /// term := (IDENT | "(" expr ")") [ ":" weight ]
    fn term(&mut self) -> Result<LayoutSpec> {
        let mut spec = match self.peek() {
            Some(b'(') => {
                self.pos += 1;
                let inner = self.expr()?;
                if !self.eat(b')') {
                    return Err(self.err("expected ')'"));
                }
                inner
            }
            Some(c) if is_ident_start(c) => LayoutSpec::panel(self.ident()?),
            Some(other) => {
                return Err(self.err(format!("expected pane name or '(', found '{}'", other as char)))
            }
            None => return Err(self.err("unexpected end of layout")),
        };
        if self.eat(b':') {
            let w = self.number()?;
            spec = match spec {
                LayoutSpec::Panel { name, .. } => LayoutSpec::Panel { name, weight: w },
                LayoutSpec::Split { dir, children, .. } => {
                    LayoutSpec::Split { dir, weight: w, children }
                }
            };
        }
        Ok(spec)
    }

    fn ident(&mut self) -> Result<String> {
        self.skip_ws();
        let start = self.pos;
        while matches!(self.bytes.get(self.pos), Some(c) if is_ident_char(*c)) {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(self.err("expected pane name"));
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| self.err("invalid utf-8"))?;
        Ok(s.to_string())
    }

    fn number(&mut self) -> Result<u16> {
        self.skip_ws();
        let start = self.pos;
        while matches!(self.bytes.get(self.pos), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(self.err("expected a weight after ':'"));
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| self.err("invalid utf-8"))?;
        s.parse::<u16>()
            .map_err(|_| self.err(format!("weight '{s}' out of range (0-65535)")))
    }
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_'
}

/// The layout used when nothing else is configured.
pub const DEFAULT_LAYOUT: &str = "(prompt:72 | (notes:58 / dashboard:42)):80 / logs:20";

/// Built-in workspace presets: `name → layout`.
pub const LAYOUT_PRESETS: &[(&str, &str)] = &[
    ("main", DEFAULT_LAYOUT),
    ("coding", "(prompt:80 | notes:20):78 / logs:22"),
    ("writing", "(prompt:65 | notes:35):85 / dashboard:15"),
    ("devops", "(prompt:70 | (logs:50 / dashboard:50)):80 / notes:20"),
    ("media", "(prompt:60 | dashboard:40):78 / notes:22"),
    ("gaming", "(prompt:85 | notes:15):85 / dashboard:15"),
    ("focus", "prompt:100"),
];

/// Look up a built-in layout preset by name.
pub fn preset(name: &str) -> Option<&'static str> {
    LAYOUT_PRESETS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_pane() {
        let s = LayoutSpec::parse("prompt").unwrap();
        assert_eq!(s.names(), vec!["prompt"]);
        assert_eq!(s.weight(), 1);
    }

    #[test]
    fn parses_weighted_columns() {
        let s = LayoutSpec::parse("prompt:72 | notes:28").unwrap();
        assert_eq!(s.names(), vec!["prompt", "notes"]);
        match s {
            LayoutSpec::Split { dir, children, .. } => {
                assert_eq!(dir, Direction::Horizontal);
                assert_eq!(children[0].weight(), 72);
                assert_eq!(children[1].weight(), 28);
            }
            LayoutSpec::Panel { .. } => panic!("expected split"),
        }
    }

    #[test]
    fn parses_nested_groups() {
        let s = LayoutSpec::parse("(prompt:72 | (notes:58 / dashboard:42)):80 / logs:20").unwrap();
        assert_eq!(s.names(), vec!["prompt", "notes", "dashboard", "logs"]);
        match s {
            LayoutSpec::Split { dir, .. } => assert_eq!(dir, Direction::Vertical),
            LayoutSpec::Panel { .. } => panic!("expected split"),
        }
    }

    #[test]
    fn compute_produces_covering_rects() {
        let s = LayoutSpec::parse(DEFAULT_LAYOUT).unwrap();
        let area = Rect::new(0, 0, 120, 40);
        let rects = s.compute(area, 0);
        assert_eq!(rects.len(), 4);
        let total: u32 = rects.iter().map(|(_, r)| u32::from(r.w) * u32::from(r.h)).sum();
        assert_eq!(total, 120 * 40);
        // prompt is the top-left pane.
        assert_eq!(rects[0].1, Rect::new(0, 0, 86, 32));
        assert_eq!(rects[3].0, "logs");
        // With a gap the panes shrink but never overlap.
        let gapped = s.compute(area, 1);
        assert_eq!(gapped.len(), 4);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(LayoutSpec::parse("").is_err());
        assert!(LayoutSpec::parse("(prompt").is_err());
        assert!(LayoutSpec::parse("prompt |").is_err());
        assert!(LayoutSpec::parse("| prompt").is_err());
        assert!(LayoutSpec::parse("prompt:x").is_err());
        assert!(LayoutSpec::parse("prompt:99999999").is_err());
        assert!(LayoutSpec::parse("prompt extra").is_err());
    }

    #[test]
    fn all_presets_parse() {
        for (name, spec) in LAYOUT_PRESETS {
            let parsed = LayoutSpec::parse(spec)
                .unwrap_or_else(|e| panic!("preset {name} failed to parse: {e}"));
            assert!(!parsed.names().is_empty(), "preset {name} has no panes");
        }
        assert!(preset("main").is_some());
        assert!(preset("nope").is_none());
    }
}
