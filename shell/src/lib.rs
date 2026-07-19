//! `slate-shell` — the desktop.
//!
//! The shell owns the event loop and the panes; everything else (commands,
//! config, themes, notes, search…) lives in the lower crates. Rendering goes
//! through `slate-term`, so the shell runs identically against a real
//! terminal ([`slate_term::AnsiBackend`]) or the in-memory
//! [`slate_term::TestBackend`] that powers `slate snapshot` and the golden
//! tests.

#![forbid(unsafe_code)]

pub mod app;
pub mod panes;
pub mod snapshot;

pub use app::App;
pub use snapshot::{render_snapshot, SnapshotOptions, SnapshotView};

use slate_command::{Ctx, Output, Registry};
use slate_core::error::Result;
use slate_core::style::Line;

/// Execute command lines headlessly (no TUI) — used by `slate run`, the
/// integration tests, and scripts. Returns the accumulated output.
pub fn run_headless(lines: &[&str], keep_going: bool) -> Result<Output> {
    let registry = Registry::builtins();
    let mut ctx = Ctx::load_headless()?;
    let mut acc = Output::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match registry.exec(trimmed, &mut ctx) {
            Ok(mut out) => {
                acc.lines.append(&mut out.lines);
                acc.effects.append(&mut out.effects);
            }
            Err(e) => {
                if keep_going {
                    acc.line(Line::styled(format!("error: {e}"), ctx.theme().error()));
                } else {
                    return Err(e);
                }
            }
        }
    }
    Ok(acc)
}
