//! A single terminal cell.

use slate_core::style::{Attrs, Color, Style};

/// One cell of the terminal grid.
///
/// For East-Asian wide characters the leading cell holds `ch` and the
/// following cell is marked [`Cell::cont`] (continuation) so the diff
/// renderer can skip it (the terminal itself consumes both cells).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: Attrs,
    /// Right half of a wide character.
    pub cont: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Cell { ch: ' ', fg: Color::Default, bg: Color::Default, attrs: Attrs::NONE, cont: false }
    }
}

impl Cell {
    pub fn with(style: Style) -> Self {
        Cell { ch: ' ', fg: style.fg, bg: style.bg, attrs: style.attrs, cont: false }
    }

    pub fn style(&self) -> Style {
        Style { fg: self.fg, bg: self.bg, attrs: self.attrs }
    }

    pub fn set_style(&mut self, style: Style) {
        self.fg = style.fg;
        self.bg = style.bg;
        self.attrs = style.attrs;
    }

    pub fn is_blank(&self) -> bool {
        self.ch == ' ' && !self.cont
    }

    pub fn reset(&mut self) {
        *self = Cell::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::style::Style;

    #[test]
    fn default_is_blank() {
        assert!(Cell::default().is_blank());
    }

    #[test]
    fn style_roundtrip() {
        let s = Style::new().fg(Color::Named(1)).bold();
        let mut c = Cell::with(s);
        assert_eq!(c.style(), s);
        c.set_style(Style::new());
        assert_eq!(c.style(), Style::new());
    }
}
