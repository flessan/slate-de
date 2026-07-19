//! `slate-term` — Slate's first-party terminal engine.
//!
//! # Why not ratatui/crossterm?
//!
//! Slate is built around *supply-chain minimalism* (see
//! `docs/adr/0001-terminal-layer.md`). The renderer is a deliberate, small,
//! fully-`std` implementation:
//!
//! * [`Buffer`] — a double-buffered cell grid with wide-character support
//! * [`Backend`] — output abstraction; [`AnsiBackend`] talks to real
//!   terminals, [`TestBackend`] powers tests and the `slate snapshot` command
//! * [`Terminal`] — owns the frame lifecycle: draw → diff → flush
//! * [`input`]/[`KeyDecoder`] — parsing of terminal input escape sequences
//! * [`widgets`] — blocks, paragraphs, lists, gauges, sparklines
//! * [`svg`] — export a buffer to SVG (README screenshots, golden tests)
//!
//! Everything is platform-independent; only the ANSI backend touches the
//! outside world, and even there only via `stty` (documented in
//! [`AnsiBackend`]).

#![forbid(unsafe_code)]

pub mod backend;
pub mod buffer;
pub mod cell;
pub mod input;
pub mod svg;
pub mod terminal;
pub mod widgets;

pub use backend::{AnsiBackend, Backend, TestBackend};
pub use buffer::Buffer;
pub use cell::Cell;
pub use input::{Key, KeyCode, KeyDecoder, Mods};
pub use slate_core::style::{Attrs, Color, Line, Span, Style};
pub use terminal::Terminal;

/// Build a [`Terminal`] backed by a [`TestBackend`] — used pervasively in
/// tests and by `slate snapshot`.
pub fn test_terminal(w: u16, h: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(w, h))
}
