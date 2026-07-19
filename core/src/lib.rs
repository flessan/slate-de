//! `slate-core` — fundamental types shared by every Slate crate.
//!
//! This crate has **zero external dependencies** and forbids `unsafe`.
//! Higher-level crates (terminal engine, command registry, shell UI) build on
//! top of the primitives defined here.
//!
//! # Contents
//!
//! * [`style`] — colors, attributes, styled text ([`Span`], [`Line`])
//! * [`rect`] — terminal geometry ([`Rect`], weight-based splits)
//! * [`layout`] — the declarative pane-layout DSL and layout engine
//! * [`workspace`] — workspace model
//! * [`effect`] — side effects produced by commands, consumed by the shell
//! * [`civil`] — dependency-free date/time math (no `chrono`)
//! * [`proc`] — subprocess helpers
//! * [`unicode`], [`url`], [`args`] — small shared utilities

#![forbid(unsafe_code)]

pub mod args;
pub mod civil;
pub mod effect;
pub mod error;
pub mod layout;
pub mod notify;
pub mod proc;
pub mod rect;
pub mod style;
pub mod unicode;
pub mod url;
pub mod workspace;

pub use error::{Error, Result};
pub use style::{Attrs, Color, Line, Span, Style};

/// Crate version (matches the workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application name used in paths, greetings, and logs.
pub const APP_NAME: &str = "slate";
