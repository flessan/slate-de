//! The cell grid: Slate's virtual screen.

use slate_core::rect::Rect;
use slate_core::style::{Line, Style};
use slate_core::unicode;

use crate::cell::Cell;

/// A rectangular grid of [`Cell`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Buffer {
    w: u16,
    h: u16,
    cells: Vec<Cell>,
}

impl Buffer {
    /// A fresh all-blank buffer.
    pub fn new(w: u16, h: u16) -> Self {
        Buffer { w, h, cells: vec![Cell::default(); usize::from(w) * usize::from(h)] }
    }

    pub fn width(&self) -> u16 {
        self.w
    }

    pub fn height(&self) -> u16 {
        self.h
    }

    pub fn area(&self) -> Rect {
        Rect::new(0, 0, self.w, self.h)
    }

    #[inline]
    fn idx(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.w) + usize::from(x)
    }

    pub fn in_bounds(&self, x: u16, y: u16) -> bool {
        x < self.w && y < self.h
    }

    /// Borrow the cell at `(x, y)`; out of bounds yields a static blank.
    pub fn get(&self, x: u16, y: u16) -> &Cell {
        if self.in_bounds(x, y) {
            &self.cells[self.idx(x, y)]
        } else {
            static BLANK: Cell = Cell {
                ch: ' ',
                fg: crate::Color::Default,
                bg: crate::Color::Default,
                attrs: crate::Attrs::NONE,
                cont: false,
            };
            &BLANK
        }
    }

    pub fn set_cell(&mut self, x: u16, y: u16, cell: Cell) {
        if self.in_bounds(x, y) {
            let i = self.idx(x, y);
            self.cells[i] = cell;
        }
    }

    /// Write a single character (handles wide chars via continuation cells).
    /// Returns the number of cells consumed.
    pub fn set_char(&mut self, x: u16, y: u16, ch: char, style: Style) -> u16 {
        if !self.in_bounds(x, y) {
            return 0;
        }
        let w = unicode::char_width(ch);
        if w == 0 {
            return 0; // zero-width: nothing to draw
        }
        let i = self.idx(x, y);
        self.cells[i] = Cell { ch, fg: style.fg, bg: style.bg, attrs: style.attrs, cont: false };
        if w == 2 {
            if x + 1 < self.w {
                let j = self.idx(x + 1, y);
                self.cells[j] =
                    Cell { ch: ' ', fg: style.fg, bg: style.bg, attrs: style.attrs, cont: true };
                return 2;
            }
            return 1; // clipped wide char at the right edge
        }
        1
    }

    /// Write a string starting at `(x, y)`, clipped to `max_width` cells and
    /// to the buffer. Returns cells consumed.
    pub fn write_str(&mut self, x: u16, y: u16, text: &str, style: Style, max_width: u16) -> u16 {
        let mut cx = x;
        let mut used = 0u16;
        for ch in text.chars() {
            let cw = unicode::char_width(ch) as u16;
            if cw == 0 {
                continue;
            }
            if used + cw > max_width || cx >= self.w {
                break;
            }
            used += self.set_char(cx, y, ch, style);
            cx = x.saturating_add(used);
        }
        used
    }

    /// Write a styled [`Line`] at `(x, y)`. Returns cells consumed.
    pub fn write_line(&mut self, x: u16, y: u16, line: &Line, max_width: u16) -> u16 {
        let mut used = 0u16;
        for span in &line.spans {
            if used >= max_width {
                break;
            }
            used += self.write_str(x + used, y, &span.text, span.style, max_width - used);
        }
        used
    }

    /// Fill `area` with blanks in `style` (clipped to the buffer).
    pub fn fill(&mut self, area: Rect, style: Style) {
        for y in area.y..area.bottom().min(self.h) {
            for x in area.x..area.right().min(self.w) {
                let i = self.idx(x, y);
                self.cells[i] = Cell::with(style);
            }
        }
    }

    /// Reset every cell to a blank with `style`.
    pub fn clear(&mut self, style: Style) {
        for c in &mut self.cells {
            *c = Cell::with(style);
        }
    }

    /// Resize, discarding content (the shell repaints every frame anyway).
    pub fn resize(&mut self, w: u16, h: u16) {
        *self = Buffer::new(w, h);
    }

    /// The plain text of row `y` (continuation cells skipped), right-trimmed.
    pub fn line_string(&self, y: u16) -> String {
        if y >= self.h {
            return String::new();
        }
        let mut s = String::with_capacity(usize::from(self.w));
        for x in 0..self.w {
            let c = self.get(x, y);
            if !c.cont {
                s.push(c.ch);
            }
        }
        s.trim_end().to_string()
    }

    /// All rows joined by `\n`, each right-trimmed.
    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        for y in 0..self.h {
            if y > 0 {
                out.push('\n');
            }
            out.push_str(&self.line_string(y));
        }
        out
    }

    /// Cells that differ from `prev` (row-major order), for the diff renderer.
    pub fn diff(&self, prev: &Buffer) -> Vec<(u16, u16, Cell)> {
        let mut out = Vec::new();
        if self.w != prev.w || self.h != prev.h {
            for y in 0..self.h {
                for x in 0..self.w {
                    out.push((x, y, self.get(x, y).clone()));
                }
            }
            return out;
        }
        for y in 0..self.h {
            for x in 0..self.w {
                let i = self.idx(x, y);
                if self.cells[i] != prev.cells[i] {
                    out.push((x, y, self.cells[i].clone()));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::style::{Color, Span};

    fn style() -> Style {
        Style::new().fg(Color::Named(2))
    }

    #[test]
    fn write_and_read_back() {
        let mut b = Buffer::new(10, 3);
        let used = b.write_str(0, 0, "hello", style(), 10);
        assert_eq!(used, 5);
        assert_eq!(b.line_string(0), "hello");
        assert_eq!(b.line_string(1), "");
    }

    #[test]
    fn wide_chars_use_continuation() {
        let mut b = Buffer::new(8, 1);
        let used = b.write_str(0, 0, "a界b", style(), 8);
        assert_eq!(used, 4);
        assert!(b.get(2, 0).cont);
        assert_eq!(b.line_string(0), "a界b");
        assert_eq!(b.to_plain_text(), "a界b");
    }

    #[test]
    fn clipping_and_max_width() {
        let mut b = Buffer::new(4, 1);
        let used = b.write_str(0, 0, "clipped!", style(), 4);
        assert_eq!(used, 4);
        assert_eq!(b.line_string(0), "clip");
        // 'a' + '界' (wide) fills the 3-cell limit exactly; 'z' is clipped.
        let mut c = Buffer::new(3, 1);
        let used = c.write_str(0, 0, "a界z", style(), 3);
        assert_eq!(used, 3);
        assert_eq!(c.line_string(0), "a界");
    }

    #[test]
    fn write_line_with_spans() {
        let mut b = Buffer::new(12, 1);
        let line = Line::new(vec![
            Span::plain("status: "),
            Span::styled("ok", Style::new().fg(Color::Named(2))),
        ]);
        b.write_line(0, 0, &line, 12);
        assert_eq!(b.line_string(0), "status: ok");
        assert_eq!(b.get(9, 0).fg, Color::Named(2));
    }

    #[test]
    fn fill_and_clear() {
        let mut b = Buffer::new(5, 2);
        b.fill(Rect::new(1, 0, 2, 1), style());
        assert_eq!(b.get(1, 0).fg, Color::Named(2));
        assert_eq!(b.get(0, 0).fg, Color::Default);
        b.clear(style());
        assert_eq!(b.get(0, 0).fg, Color::Named(2));
    }

    #[test]
    fn diff_reports_changes_only() {
        let mut a = Buffer::new(4, 2);
        let b_prev = a.clone();
        a.set_cell(1, 1, Cell { ch: 'x', ..Cell::default() });
        let d = a.diff(&b_prev);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, 1);
        assert_eq!(d[0].1, 1);
    }

    #[test]
    fn diff_size_mismatch_full() {
        let a = Buffer::new(4, 2);
        let prev = Buffer::new(2, 2);
        assert_eq!(a.diff(&prev).len(), 8);
    }

    #[test]
    fn out_of_bounds_is_safe() {
        let mut b = Buffer::new(2, 2);
        b.set_cell(9, 9, Cell { ch: 'x', ..Cell::default() });
        assert_eq!(b.get(9, 9).ch, ' ');
        assert_eq!(b.line_string(9), "");
        assert_eq!(b.set_char(1, 1, '界', Style::new()), 1); // clipped wide char
    }
}
