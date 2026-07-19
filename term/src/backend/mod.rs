//! Output backends.
//!
//! A [`Backend`] receives *diffs* (changed cells) and a cursor position. The
//! ANSI implementation byte-optimizes SGR/cursor sequences; the test
//! implementation reconstructs the screen for assertions and SVG export.

use std::io;

use crate::cell::Cell;

mod ansi;
mod test;

pub use ansi::{probe_size, AnsiBackend, ColorDepth, RawMode, TerminalSession};
pub use test::TestBackend;

/// Terminal output abstraction.
pub trait Backend {
    /// Current terminal size `(cols, rows)`. Called once per frame so
    /// resizes (SIGWINCH-free polling) are picked up automatically.
    fn size(&mut self) -> (u16, u16);

    /// Apply a batch of changed cells (row-major order).
    fn draw(&mut self, changes: &[(u16, u16, Cell)]) -> io::Result<()>;

    /// Clear the whole screen and move the cursor home.
    fn clear(&mut self) -> io::Result<()>;

    /// Position the visible cursor (`None` hides it).
    fn set_cursor(&mut self, pos: Option<(u16, u16)>) -> io::Result<()>;

    /// Flush any buffered output.
    fn flush(&mut self) -> io::Result<()>;
}
