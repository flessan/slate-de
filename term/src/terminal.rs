//! The frame lifecycle: draw every frame into a fresh buffer, diff against
//! the previous frame, push only changes to the backend.

use std::io;

use slate_core::rect::Rect;

use crate::backend::Backend;
use crate::buffer::Buffer;

/// Owns the double buffer and drives a [`Backend`].
pub struct Terminal<B: Backend> {
    backend: B,
    next: Buffer,
    prev: Buffer,
    cursor: Option<(u16, u16)>,
}

impl<B: Backend> Terminal<B> {
    pub fn new(mut backend: B) -> Self {
        let (w, h) = backend.size();
        Terminal { backend, next: Buffer::new(w, h), prev: Buffer::new(w, h), cursor: None }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// The drawable area of the current frame.
    pub fn area(&self) -> Rect {
        self.next.area()
    }

    /// Show the cursor at `(x, y)` for the next frame (`None` hides it).
    pub fn set_cursor_position(&mut self, pos: Option<(u16, u16)>) {
        self.cursor = pos;
    }

    /// Re-query size; resize buffers and force a full repaint when changed.
    pub fn sync_size(&mut self) -> io::Result<bool> {
        let (w, h) = self.backend.size();
        let changed = (w, h) != (self.next.width(), self.next.height());
        if changed {
            self.next.resize(w, h);
            self.prev.resize(w, h);
            self.backend.clear()?;
        }
        Ok(changed)
    }

    /// Build a frame with `render`, diff it, and flush the changes.
    pub fn draw<F>(&mut self, render: F) -> io::Result<()>
    where
        F: FnOnce(&mut Buffer, Rect),
    {
        // Each frame starts blank so panes never see two-frame-old content.
        self.next.clear(Default::default());
        render(&mut self.next, self.next.area());
        let changes = self.next.diff(&self.prev);
        self.backend.draw(&changes)?;
        self.backend.set_cursor(self.cursor)?;
        self.backend.flush()?;
        std::mem::swap(&mut self.next, &mut self.prev);
        Ok(())
    }

    /// Frame count is derivable from backend-specific counters; exposed for
    /// tests through backends themselves.
    pub fn buffered_text(&self) -> String
    where
        B: AsTestBackend,
    {
        self.backend.test_buffer_text()
    }
}

/// Helper trait exposing a backend's reconstructed screen (implemented by
/// [`crate::TestBackend`]) so integration tests can assert on real frames.
pub trait AsTestBackend: Backend {
    fn test_buffer_text(&self) -> String;
}

impl AsTestBackend for crate::backend::TestBackend {
    fn test_buffer_text(&self) -> String {
        self.buffer().to_plain_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::TestBackend;

    #[test]
    fn frames_diff_and_commit() {
        let mut term = Terminal::new(TestBackend::new(6, 2));
        term.draw(|buf, _| {
            buf.write_str(0, 0, "hello", Default::default(), 6);
        })
        .unwrap();
        assert_eq!(term.backend().buffer().line_string(0), "hello");
        assert_eq!(term.backend().draw_count(), 1);

        // Second frame draws more: diffs accumulate onto the same screen.
        term.draw(|buf, _| {
            buf.write_str(0, 0, "hello", Default::default(), 6);
            buf.write_str(0, 1, "2nd", Default::default(), 6);
        })
        .unwrap();
        assert_eq!(term.backend().buffer().line_string(0), "hello");
        assert_eq!(term.backend().buffer().line_string(1), "2nd");
    }

    #[test]
    fn clear_flag_forces_repaint() {
        let mut term = Terminal::new(TestBackend::new(6, 2));
        term.draw(|buf, _| {
            buf.write_str(0, 0, "abc", Default::default(), 6);
        })
        .unwrap();
        // Simulate an external size change: nothing to resize with a fixed
        // backend, but `sync_size` must report `unchanged`.
        assert!(!term.sync_size().unwrap());
    }
}
