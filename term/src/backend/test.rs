//! In-memory backend for tests, snapshots, and SVG export.

use std::io;

use super::Backend;
use crate::buffer::Buffer;
use crate::cell::Cell;

/// Reconstructs the virtual screen from the emitted diffs — which also means
/// every snapshot test exercises the real diff pipeline.
#[derive(Debug)]
pub struct TestBackend {
    w: u16,
    h: u16,
    committed: Buffer,
    cursor: Option<(u16, u16)>,
    draws: usize,
}

impl TestBackend {
    pub fn new(w: u16, h: u16) -> Self {
        TestBackend { w, h, committed: Buffer::new(w, h), cursor: None, draws: 0 }
    }

    /// The reconstructed screen.
    pub fn buffer(&self) -> &Buffer {
        &self.committed
    }

    /// Last cursor position (if shown).
    pub fn cursor(&self) -> Option<(u16, u16)> {
        self.cursor
    }

    /// Number of cycles through [`Backend::draw`].
    pub fn draw_count(&self) -> usize {
        self.draws
    }
}

impl Backend for TestBackend {
    fn size(&mut self) -> (u16, u16) {
        (self.w, self.h)
    }

    fn draw(&mut self, changes: &[(u16, u16, Cell)]) -> io::Result<()> {
        self.draws += 1;
        for (x, y, cell) in changes {
            self.committed.set_cell(*x, *y, cell.clone());
        }
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.committed = Buffer::new(self.w, self.h);
        Ok(())
    }

    fn set_cursor(&mut self, pos: Option<(u16, u16)>) -> io::Result<()> {
        self.cursor = pos;
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::style::{Color, Style};

    #[test]
    fn reconstructs_screen() {
        let mut b = TestBackend::new(5, 2);
        let cell = Cell { ch: 'z', fg: Color::Named(3), ..Cell::default() };
        b.draw(&[(4, 1, cell)]).unwrap();
        assert_eq!(b.buffer().line_string(1), "    z");
        assert_eq!(b.buffer().get(4, 1).fg, Color::Named(3));
    }

    #[test]
    fn reset_on_clear() {
        let mut b = TestBackend::new(5, 1);
        b.committed.fill(b.committed.area(), Style::new().bg(Color::Named(1)));
        b.clear().unwrap();
        assert_eq!(b.buffer().get(0, 0).bg, Color::Default);
    }
}
