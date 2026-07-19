//! ANSI/VT output backend.
//!
//! Raw mode is configured through the system's `stty(1)` instead of a libc
//! dependency (see `docs/adr/0001-terminal-layer.md`). This is the only
//! platform glue in the workspace; a future `slate-term` feature may swap it
//! for direct syscalls without touching any other code.

use std::io::{self, Write};

use slate_core::proc;
use slate_core::style::{Attrs, Color, Style};

use super::Backend;
use crate::cell::Cell;

/// Guard that restores the terminal to its previous mode on drop.
pub struct RawMode {
    saved: Option<String>,
    active: bool,
}

impl RawMode {
    /// Save current tty settings and switch to raw mode (`stty raw -echo -ixon`).
    pub fn enable() -> Self {
        let saved = proc::run_trimmed("stty", &["-g"]);
        let ok = proc::run("stty", &["raw", "-echo", "-ixon"]).map(|o| o.status.success()).unwrap_or(false);
        RawMode { saved: if ok { saved } else { None }, active: ok }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        match &self.saved {
            Some(state) => {
                let _ = proc::run("stty", &[state.as_str()]);
            }
            None => {
                let _ = proc::run("stty", &["sane"]);
            }
        }
    }
}

/// How many colors the terminal claims to support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorDepth {
    /// 16 palette entries (ANSI).
    Basic,
    /// The xterm 256-color palette.
    Extended,
    /// 24-bit true color.
    TrueColor,
}

impl ColorDepth {
    /// Detect from `COLORTERM` / `TERM`.
    pub fn detect() -> Self {
        match std::env::var("COLORTERM").unwrap_or_default().as_str() {
            "truecolor" | "24bit" => return ColorDepth::TrueColor,
            _ => {}
        }
        let term = std::env::var("TERM").unwrap_or_default();
        if term.contains("256color") {
            ColorDepth::Extended
        } else if term.is_empty() || term == "dumb" {
            ColorDepth::Basic
        } else {
            ColorDepth::Extended
        }
    }
}

/// Streams cells to a writer using ANSI escape sequences.
pub struct AnsiBackend<W: Write> {
    out: W,
    size: (u16, u16),
    depth: ColorDepth,
    cur_style: Option<Style>,
    cursor_hidden: bool,
}

impl<W: Write> AnsiBackend<W> {
    pub fn new(out: W) -> Self {
        AnsiBackend {
            out,
            size: probe_size(),
            depth: ColorDepth::detect(),
            cur_style: None,
            cursor_hidden: false,
        }
    }

    pub fn with_depth(mut self, depth: ColorDepth) -> Self {
        self.depth = depth;
        self
    }

    /// Enter the alternate screen (call once at session start).
    pub fn enter_altscreen(&mut self) -> io::Result<()> {
        self.out.write_all(b"\x1b[?1049h\x1b[2J\x1b[H")
    }

    /// Leave the alternate screen (call once at session end).
    pub fn leave_altscreen(&mut self) -> io::Result<()> {
        self.out.write_all(b"\x1b[?1049l")
    }

    pub fn hide_cursor(&mut self) -> io::Result<()> {
        self.cursor_hidden = true;
        self.out.write_all(b"\x1b[?25l")
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.cursor_hidden = false;
        self.out.write_all(b"\x1b[?25h")
    }

    fn move_to(&mut self, x: u16, y: u16) -> io::Result<()> {
        write!(self.out, "\x1b[{};{}H", y + 1, x + 1)
    }

    /// Emit SGR when `style` differs from the previously emitted style.
    fn apply_style(&mut self, style: Style) -> io::Result<()> {
        if self.cur_style == Some(style) {
            return Ok(());
        }
        self.cur_style = Some(style);
        let mut codes: Vec<String> = vec!["0".to_string()];
        push_color_codes(&mut codes, style.fg, self.depth, false);
        push_color_codes(&mut codes, style.bg, self.depth, true);
        if style.attrs.contains(Attrs::BOLD) {
            codes.push("1".into());
        }
        if style.attrs.contains(Attrs::DIM) {
            codes.push("2".into());
        }
        if style.attrs.contains(Attrs::ITALIC) {
            codes.push("3".into());
        }
        if style.attrs.contains(Attrs::UNDERLINE) {
            codes.push("4".into());
        }
        if style.attrs.contains(Attrs::REVERSE) {
            codes.push("7".into());
        }
        if style.attrs.contains(Attrs::CROSSED) {
            codes.push("9".into());
        }
        write!(self.out, "\x1b[{}m", codes.join(";"))
    }
}

impl<W: Write> Backend for AnsiBackend<W> {
    fn size(&mut self) -> (u16, u16) {
        self.size = probe_size();
        self.size
    }

    fn draw(&mut self, changes: &[(u16, u16, Cell)]) -> io::Result<()> {
        let mut pos: Option<(u16, u16)> = None;
        let mut last_wide: Option<(u16, u16)> = None;
        for (x, y, cell) in changes {
            // Continuation halves of wide glyphs are emitted implicitly by
            // the terminal when the leading cell is written.
            if cell.cont && last_wide == Some((x.saturating_sub(1), *y)) {
                continue;
            }
            let style = cell.style();
            if pos != Some((*x, *y)) {
                self.move_to(*x, *y)?;
            }
            self.apply_style(style)?;
            let mut buf = [0u8; 4];
            let n = if cell.cont { 0 } else { cell.ch.encode_utf8(&mut buf).len() };
            let bytes: &[u8] = if cell.cont { b" " } else { &buf[..n] };
            self.out.write_all(bytes)?;
            let width = slate_core::unicode::char_width(cell.ch) as u16;
            pos = Some((x.saturating_add(width.max(1)), *y));
            last_wide = if width == 2 { Some((*x, *y)) } else { None };
        }
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.cur_style = None;
        self.out.write_all(b"\x1b[0m\x1b[2J\x1b[H")
    }

    fn set_cursor(&mut self, pos: Option<(u16, u16)>) -> io::Result<()> {
        match pos {
            Some((x, y)) => {
                if self.cursor_hidden {
                    self.show_cursor()?;
                }
                self.move_to(x, y)
            }
            None => {
                if !self.cursor_hidden {
                    self.hide_cursor()
                } else {
                    Ok(())
                }
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

fn push_color_codes(codes: &mut Vec<String>, color: Color, depth: ColorDepth, bg: bool) {
    // Default palette translation keeps the 16 base colors native so user
    // terminal themes still work; extended/truecolor go through 38;5 / 38;2.
    match color {
        Color::Default => codes.push(if bg { "49".into() } else { "39".into() }),
        Color::Named(n) if n < 8 => codes.push(format!("{}", if bg { 40 + n } else { 30 + n })),
        Color::Named(n) if n < 16 => {
            codes.push(format!("{}", if bg { 100 + n - 8 } else { 90 + n - 8 }))
        }
        Color::Named(n) => {
            push_numeric(codes, n, bg, depth);
        }
        Color::Indexed(n) => push_numeric(codes, n, bg, depth),
        Color::Rgb(r, g, b) => match depth {
            ColorDepth::TrueColor => {
                codes.push(format!("{};2;{};{};{}", if bg { 48 } else { 38 }, r, g, b));
            }
            _ => push_numeric(codes, rgb_to_256(r, g, b), bg, depth),
        },
    }
}

fn push_numeric(codes: &mut Vec<String>, n: u8, bg: bool, _depth: ColorDepth) {
    codes.push(format!("{};5;{}", if bg { 48 } else { 38 }, n));
}

/// Approximate an RGB color in the xterm 256 palette.
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b {
        // grayscale ramp: 232..=255
        if r < 8 {
            return 16;
        }
        if r > 238 {
            return 231;
        }
        return (((r - 8) as u16 * 24 / 230) as u8) + 232;
    }
    let q = |v: u8| (u16::from(v) * 5 / 255) as u8;
    16 + 36 * q(r) + 6 * q(g) + q(b)
}

/// Probe terminal size: `stty size`, then `LINES`/`COLUMNS`, then 80x24.
pub fn probe_size() -> (u16, u16) {
    if let Some(s) = proc::run_trimmed("stty", &["size"]) {
        let mut it = s.split_whitespace();
        if let (Some(rows), Some(cols)) = (it.next(), it.next()) {
            if let (Ok(r), Ok(c)) = (rows.parse::<u16>(), cols.parse::<u16>()) {
                if r > 0 && c > 0 {
                    return (c, r);
                }
            }
        }
    }
    let from_env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u16>().ok());
    match (from_env("COLUMNS"), from_env("LINES")) {
        (Some(c), Some(r)) if c > 0 && r > 0 => (c, r),
        _ => (80, 24),
    }
}

/// Combine a RawMode guard with an alt-screened backend; the canonical way
/// to run the interactive shell.
pub struct TerminalSession<W: Write> {
    _raw: RawMode,
    pub backend: AnsiBackend<W>,
}

impl<W: Write> TerminalSession<W> {
    pub fn begin(out: W) -> io::Result<Self> {
        let raw = RawMode::enable();
        if !raw.is_active() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "could not enable raw mode (no tty?). Try `slate run <cmd>` instead",
            ));
        }
        let mut backend = AnsiBackend::new(out);
        backend.enter_altscreen()?;
        backend.hide_cursor()?;
        backend.flush()?;
        Ok(TerminalSession { _raw: raw, backend })
    }
}

impl<W: Write> Drop for TerminalSession<W> {
    fn drop(&mut self) {
        let _ = self.backend.set_cursor(None);
        let _ = self.backend.show_cursor();
        let _ = self.backend.leave_altscreen();
        let _ = self.backend.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::style::{Attrs, Style};

    fn backend() -> AnsiBackend<Vec<u8>> {
        AnsiBackend::new(Vec::new()).with_depth(ColorDepth::TrueColor)
    }

    fn rendered(b: &AnsiBackend<Vec<u8>>) -> String {
        String::from_utf8_lossy(&b.out).to_string()
    }

    #[test]
    fn writes_cells_with_styles() {
        let mut b = backend();
        let style = Style::new().fg(Color::Named(2)).attrs(Attrs::BOLD);
        let cell = Cell { ch: 'x', fg: style.fg, bg: style.bg, attrs: style.attrs, cont: false };
        b.draw(&[(3, 2, cell.clone())]).unwrap();
        let s = rendered(&b);
        assert!(s.contains("\x1b[3;4H")); // 1-based cursor position
        assert!(s.contains("32")); // green fg
        assert!(s.contains('1') && s.contains('x'));
    }

    #[test]
    fn skips_continuation_after_wide() {
        let mut b = backend();
        let lead = Cell { ch: '界', ..Cell::default() };
        let cont = Cell { ch: ' ', cont: true, ..Cell::default() };
        let plain = Cell { ch: 'a', ..Cell::default() };
        b.draw(&[(0, 0, lead), (1, 0, cont), (2, 0, plain)]).unwrap();
        let s = rendered(&b);
        assert!(s.contains("界a"));
        assert!(!s.contains("界 a")); // no space between wide glyph and next cell
    }

    #[test]
    fn truecolor_and_fallback() {
        let mut codes = Vec::new();
        push_color_codes(&mut codes, Color::Rgb(255, 0, 0), ColorDepth::TrueColor, false);
        assert_eq!(codes, vec!["38;2;255;0;0"]);
        let mut codes = Vec::new();
        push_color_codes(&mut codes, Color::Rgb(255, 0, 0), ColorDepth::Extended, false);
        assert_eq!(codes, vec!["38;5;196"]);
    }

    #[test]
    fn rgb_to_256_known_values() {
        assert_eq!(rgb_to_256(255, 0, 0), 196);
        assert_eq!(rgb_to_256(0, 0, 0), 16);
        assert_eq!(rgb_to_256(128, 128, 128), 244);
        assert_eq!(rgb_to_256(255, 255, 255), 231);
    }

    #[test]
    fn style_dedup() {
        let mut b = backend();
        let s = Style::new().fg(Color::Named(1));
        b.apply_style(s).unwrap();
        let len1 = rendered(&b).len();
        b.apply_style(s).unwrap(); // no-op
        assert_eq!(rendered(&b).len(), len1);
    }

    #[test]
    fn cursor_visibility() {
        let mut b = backend();
        b.set_cursor(Some((1, 1))).unwrap();
        assert!(rendered(&b).contains("\x1b[?25h"));
        b.set_cursor(None).unwrap();
        assert!(rendered(&b).contains("\x1b[?25l"));
    }
}
