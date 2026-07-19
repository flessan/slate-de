//! Deterministic off-screen rendering of the real UI.
//!
//! `slate snapshot` renders the desktop through [`slate_term::TestBackend`]
//! exactly as if attached to a terminal, then exports to SVG or plain text.
//! README screenshots, golden tests, and user bug reports all come from this
//! one code path — so they can never drift from the real UI.

use slate_command::{Ctx, Registry};
use slate_core::error::Result;
use slate_term::svg::{buffer_to_svg, SvgOptions};

use crate::app::App;

/// What to render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotView {
    /// The default desktop with the welcome/about output.
    Desktop,
    /// Desktop with the universal search overlay open.
    Search,
    /// Notes pane focused with seeded content.
    Notes,
    /// Desktop rendered with the ASCII theme.
    Ascii,
    /// Dashboard-focused demo (`demo` widgets).
    Dashboard,
}

impl SnapshotView {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "desktop" => Some(SnapshotView::Desktop),
            "search" => Some(SnapshotView::Search),
            "notes" => Some(SnapshotView::Notes),
            "ascii" => Some(SnapshotView::Ascii),
            "dashboard" => Some(SnapshotView::Dashboard),
            _ => None,
        }
    }

    pub fn names() -> &'static [&'static str] {
        &["desktop", "search", "notes", "ascii", "dashboard"]
    }
}

/// Rendering options.
pub struct SnapshotOptions {
    pub view: SnapshotView,
    pub width: u16,
    pub height: u16,
    /// Emit the plain-text screen instead of SVG.
    pub text: bool,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        SnapshotOptions { view: SnapshotView::Desktop, width: 120, height: 34, text: false }
    }
}

/// Build the deterministic demo context used by snapshots and golden tests.
pub fn demo_ctx() -> Ctx {
    let mut ctx = Ctx::ephemeral();
    ctx.headless = false;
    ctx.demo = true;
    ctx.notes.write("welcome", "# welcome\n\nEverything is a command. #slate").ok();
    ctx.notes.write("ideas", "# ideas\n\n- a desktop that is a shell\n- searchable everything").ok();
    ctx.notes.set_pinned("ideas", true).ok();
    ctx
}

/// Build the app for a given view (separated for golden tests).
pub fn snapshot_app(view: SnapshotView) -> App {
    let mut ctx = demo_ctx();
    if view == SnapshotView::Ascii {
        let _ = ctx.themes.set_current("ascii");
    }
    let mut app = App::new(ctx, Registry::builtins());
    match view {
        SnapshotView::Desktop | SnapshotView::Ascii => {
            let _ = app.run_command_line("about");
        }
        SnapshotView::Dashboard => {
            let _ = app.run_command_line("dashboard");
        }
        SnapshotView::Notes => {
            let _ = app.run_command_line("notes");
        }
        SnapshotView::Search => {
            let _ = app.run_command_line("about");
            app.open_search("the");
        }
    }
    app
}

/// Render to a string (SVG by default, plain text for `--text`).
pub fn render_snapshot(opts: &SnapshotOptions) -> Result<String> {
    let mut app = snapshot_app(opts.view);
    let mut term = slate_term::test_terminal(opts.width, opts.height);
    term.draw(|buf, area| app.draw_frame(buf, area))?;
    let buffer = term.backend().buffer();
    if opts.text {
        return Ok(buffer.to_plain_text());
    }
    Ok(buffer_to_svg(buffer, &SvgOptions::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_view_renders() {
        for name in SnapshotView::names() {
            let view = SnapshotView::parse(name).unwrap();
            let svg = render_snapshot(&SnapshotOptions { view, width: 100, height: 30, text: false })
                .unwrap();
            assert!(svg.starts_with("<svg"), "{name}: not svg");
            assert!(svg.ends_with("</svg>\n"), "{name}: unterminated svg");
            assert!(svg.len() > 500, "{name}: suspiciously small");
        }
    }

    #[test]
    fn text_mode_shows_ui_structure() {
        let text = render_snapshot(&SnapshotOptions {
            view: SnapshotView::Desktop,
            width: 100,
            height: 30,
            text: true,
        })
        .unwrap();
        assert!(text.contains("slate"), "{text}");
        assert!(text.contains("prompt"), "{text}");
        assert!(text.contains("notes"), "{text}");
        // The `about` output is visible in the transcript.
        assert!(text.contains("command line"), "{text}");
    }

    #[test]
    fn ascii_view_uses_ascii_borders() {
        let text = render_snapshot(&SnapshotOptions {
            view: SnapshotView::Ascii,
            width: 80,
            height: 20,
            text: true,
        })
        .unwrap();
        assert!(text.contains('+'), "{text}");
        assert!(!text.contains('╭'), "{text}");
    }

    #[test]
    fn snapshots_are_deterministic() {
        let a = render_snapshot(&SnapshotOptions { view: SnapshotView::Desktop, ..Default::default() }).unwrap();
        let b = render_snapshot(&SnapshotOptions { view: SnapshotView::Desktop, ..Default::default() }).unwrap();
        assert_eq!(a, b);
    }
}
