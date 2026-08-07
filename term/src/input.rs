//! Terminal input: byte stream → key events.
//!
//! The decoder is incremental (`feed` arbitrary chunks), allocation-light,
//! and covers the sequences emitted by mainstream terminal emulators:
//! printable text (UTF-8), control keys, arrows, Home/End/Delete/PageUp/
//! PageDown, Shift-Tab, xterm modifier encoding (`CSI 1;2x`), SS3 arrows,
//! and Alt+key (ESC prefix).

/// Modifier keys held during an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { ctrl: false, alt: false, shift: false };
    pub const CTRL: Mods = Mods { ctrl: true, alt: false, shift: false };
    pub const ALT: Mods = Mods { ctrl: false, alt: true, shift: false };

    pub fn is_none(&self) -> bool {
        !self.ctrl && !self.alt && !self.shift
    }
}

/// What was pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Delete,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

/// One decoded key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub mods: Mods,
}

impl Key {
    pub const fn plain(code: KeyCode) -> Self {
        Key { code, mods: Mods::NONE }
    }

    pub const fn char(c: char) -> Self {
        Key { code: KeyCode::Char(c), mods: Mods::NONE }
    }

    pub const fn ctrl(c: char) -> Self {
        Key { code: KeyCode::Char(c), mods: Mods::CTRL }
    }

    pub const fn alt(c: char) -> Self {
        Key { code: KeyCode::Char(c), mods: Mods::ALT }
    }

    pub const ENTER: Key = Key::plain(KeyCode::Enter);
    pub const ESC: Key = Key::plain(KeyCode::Esc);
    pub const TAB: Key = Key::plain(KeyCode::Tab);
    pub const BACKSPACE: Key = Key::plain(KeyCode::Backspace);

    /// `Ctrl+?` with a char code.
    pub fn is_ctrl(&self, c: char) -> bool {
        self.code == KeyCode::Char(c) && self.mods.ctrl && !self.mods.alt
    }

    /// `Alt+?` with a char code.
    pub fn is_alt(&self, c: char) -> bool {
        self.code == KeyCode::Char(c) && self.mods.alt && !self.mods.ctrl
    }

    /// Plain text input (no ctrl/alt).
    pub fn as_char(&self) -> Option<char> {
        match self.code {
            KeyCode::Char(c) if self.mods.is_none() => Some(c),
            _ => None,
        }
    }
}

/// Incremental escape-sequence decoder.
#[derive(Default)]
pub struct KeyDecoder {
    buf: Vec<u8>,
}

impl KeyDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw bytes; returns every complete key parsed.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Key> {
        self.buf.extend_from_slice(bytes);
        let mut keys = Vec::new();
        loop {
            match self.parse_one() {
                ParseResult::Key(k, n) => {
                    keys.push(k);
                    self.buf.drain(..n);
                }
                // Incomplete sequence: keep buffering (bounded).
                ParseResult::Incomplete => {
                    if self.buf.len() > 32 {
                        // Give up on garbage: drop one byte.
                        self.buf.drain(..1);
                    } else {
                        break;
                    }
                }
                ParseResult::Empty => break,
            }
            if self.buf.is_empty() {
                break;
            }
        }
        keys
    }

    /// Flush leftovers as plain text (call on shutdown).
    pub fn take_remaining(&mut self) -> Vec<Key> {
        let mut out = Vec::new();
        for b in self.buf.drain(..) {
            if b.is_ascii() && !b.is_ascii_control() {
                out.push(Key::char(b as char));
            }
        }
        out
    }
}

enum ParseResult {
    Key(Key, usize),
    Incomplete,
    Empty,
}

impl KeyDecoder {
    fn parse_one(&self) -> ParseResult {
        let Some((&b0, rest)) = self.buf.split_first() else {
            return ParseResult::Empty;
        };
        match b0 {
            0x00 => ParseResult::Key(Key::ctrl(' '), 1), // Ctrl+Space / Ctrl+@
            0x08 => ParseResult::Key(Key::BACKSPACE, 1),
            0x01..=0x1A => {
                // Control key or a C0 with special meaning on its own.
                match b0 {
                    0x09 => ParseResult::Key(Key::TAB, 1),
                    0x0D | 0x0A => ParseResult::Key(Key::ENTER, 1),
                    _ => {
                        let c = (b'a' + b0 - 1) as char;
                        ParseResult::Key(Key::ctrl(c), 1)
                    }
                }
            }
            0x1B => self.parse_escape(rest),
            0x7F => ParseResult::Key(Key::BACKSPACE, 1),
            _ => self.parse_utf8(),
        }
    }

    fn parse_utf8(&self) -> ParseResult {
        let b0 = self.buf[0];
        let len = if b0 < 0x80 {
            1
        } else if b0 < 0xE0 {
            2
        } else if b0 < 0xF0 {
            3
        } else {
            4
        };
        if self.buf.len() < len {
            return ParseResult::Incomplete;
        }
        match std::str::from_utf8(&self.buf[..len]) {
            Ok(s) => match s.chars().next() {
                Some(c) => ParseResult::Key(Key::char(c), len),
                None => ParseResult::Empty,
            },
            Err(_) => ParseResult::Key(Key::char('\u{FFFD}'), 1), // skip invalid byte
        }
    }

    fn parse_escape(&self, rest: &[u8]) -> ParseResult {
        let Some((&b1, rest)) = rest.split_first() else {
            // Lone ESC (chunk boundary). Treat as Escape — alt-sequences
            // practically always arrive in the same read().
            return ParseResult::Key(Key::ESC, 1);
        };
        match b1 {
            b'[' => self.parse_csi(rest),
            b'O' => {
                // SS3: application-mode arrows and Home/End.
                let Some((&b2, _)) = rest.split_first() else {
                    return ParseResult::Incomplete;
                };
                let code = match b2 {
                    b'A' => KeyCode::Up,
                    b'B' => KeyCode::Down,
                    b'C' => KeyCode::Right,
                    b'D' => KeyCode::Left,
                    b'H' => KeyCode::Home,
                    b'F' => KeyCode::End,
                    _ => return ParseResult::Key(Key::ESC, 1), // unknown; recover
                };
                ParseResult::Key(Key::plain(code), 3)
            }
            0x1B => ParseResult::Key(Key::ESC, 1), // ESC ESC…: emit one Escape
            _ => {
                // Alt+key (ASCII); non-ASCII alt-combos decode as Esc+char.
                if b1.is_ascii() && !b1.is_ascii_control() {
                    ParseResult::Key(Key::alt(b1 as char), 2)
                } else {
                    ParseResult::Key(Key::ESC, 1)
                }
            }
        }
    }

    /// CSI: ESC '[' [params] [final byte 0x40..=0x7E]
    fn parse_csi(&self, rest: &[u8]) -> ParseResult {
        let mut params: Vec<u16> = Vec::new();
        let mut cur: Option<u16> = None;
        let mut consumed = 2; // ESC '['
        for &b in rest {
            consumed += 1;
            match b {
                b'0'..=b'9' => {
                    cur = Some(cur.unwrap_or(0) * 10 + u16::from(b - b'0'));
                }
                b';' => {
                    params.push(cur.unwrap_or(1));
                    cur = None;
                }
                0x40..=0x7E => {
                    params.push(cur.unwrap_or(1));
                    return ParseResult::Key(csi_key(b, &params), consumed);
                }
                _ => return ParseResult::Key(Key::ESC, 2), // malformed; recover
            }
            if consumed > 24 {
                return ParseResult::Key(Key::ESC, 2);
            }
        }
        ParseResult::Incomplete
    }
}

fn mods_from_param(v: u16) -> Mods {
    // xterm: 1 + bitmask(shift=1, alt=2, ctrl=4)
    let bits = v.saturating_sub(1);
    Mods { shift: bits & 1 != 0, alt: bits & 2 != 0, ctrl: bits & 4 != 0 }
}

fn csi_key(final_byte: u8, params: &[u16]) -> Key {
    let nominal = *params.first().unwrap_or(&1);
    let mods = mods_from_param(*params.get(1).unwrap_or(&1));
    let with = |code| Key { code, mods };
    match final_byte {
        b'A' => with(KeyCode::Up),
        b'B' => with(KeyCode::Down),
        b'C' => with(KeyCode::Right),
        b'D' => with(KeyCode::Left),
        b'H' => with(KeyCode::Home),
        b'F' => with(KeyCode::End),
        b'Z' => Key { code: KeyCode::BackTab, mods: Mods { shift: true, ..Mods::NONE } },
        b'~' => match nominal {
            1 | 7 => with(KeyCode::Home),
            3 => with(KeyCode::Delete),
            4 | 8 => with(KeyCode::End),
            5 => with(KeyCode::PageUp),
            6 => with(KeyCode::PageDown),
            _ => Key::ESC,
        },
        _ => Key::ESC,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> Vec<Key> {
        let mut d = KeyDecoder::new();
        d.feed(bytes)
    }

    #[test]
    fn plain_text() {
        assert_eq!(decode(b"abc"), vec![Key::char('a'), Key::char('b'), Key::char('c')]);
        assert_eq!(decode("héllo界".as_bytes()).len(), 6);
        assert_eq!(decode("界".as_bytes())[0], Key::char('界'));
    }

    #[test]
    fn enter_tab_backspace() {
        assert_eq!(decode(b"\r")[0], Key::ENTER);
        assert_eq!(decode(b"\n")[0], Key::ENTER);
        assert_eq!(decode(b"\t")[0], Key::TAB);
        assert_eq!(decode(b"\x7f")[0], Key::BACKSPACE);
        assert_eq!(decode(b"\x08")[0], Key::BACKSPACE);
    }

    #[test]
    fn ctrl_letters() {
        assert_eq!(decode(b"\x01")[0], Key::ctrl('a'));
        assert_eq!(decode(b"\x0c")[0], Key::ctrl('l'));
        assert_eq!(decode(b"\x15")[0], Key::ctrl('u'));
        assert_eq!(decode(b"\x00")[0], Key::ctrl(' '));
    }

    #[test]
    fn arrows_and_navigation() {
        assert_eq!(decode(b"\x1b[A")[0], Key::plain(KeyCode::Up));
        assert_eq!(decode(b"\x1b[B")[0], Key::plain(KeyCode::Down));
        assert_eq!(decode(b"\x1b[C")[0], Key::plain(KeyCode::Right));
        assert_eq!(decode(b"\x1b[D")[0], Key::plain(KeyCode::Left));
        assert_eq!(decode(b"\x1b[H")[0], Key::plain(KeyCode::Home));
        assert_eq!(decode(b"\x1b[F")[0], Key::plain(KeyCode::End));
        assert_eq!(decode(b"\x1b[3~")[0], Key::plain(KeyCode::Delete));
        assert_eq!(decode(b"\x1b[5~")[0], Key::plain(KeyCode::PageUp));
        assert_eq!(decode(b"\x1b[6~")[0], Key::plain(KeyCode::PageDown));
        // SS3 application mode.
        assert_eq!(decode(b"\x1bOA")[0], Key::plain(KeyCode::Up));
    }

    #[test]
    fn modified_arrows() {
        let k = decode(b"\x1b[1;5A")[0]; // Ctrl+Up
        assert_eq!(k.code, KeyCode::Up);
        assert!(k.mods.ctrl && !k.mods.alt && !k.mods.shift);
        let k = decode(b"\x1b[1;2D")[0]; // Shift+Left
        assert_eq!(k.code, KeyCode::Left);
        assert!(k.mods.shift);
        let k = decode(b"\x1b[1;4C")[0]; // Alt+Right
        assert!(k.mods.alt);
    }

    #[test]
    fn backtab() {
        let k = decode(b"\x1b[Z")[0];
        assert_eq!(k.code, KeyCode::BackTab);
        assert!(k.mods.shift);
    }

    #[test]
    fn alt_chars() {
        assert_eq!(decode(b"\x1bj")[0], Key::alt('j'));
        assert_eq!(decode(b"\x1bk")[0], Key::alt('k'));
        assert_eq!(decode(b"\x1b1")[0], Key::alt('1'));
    }

    #[test]
    fn lone_escape() {
        assert_eq!(decode(b"\x1b")[0], Key::ESC);
    }

    #[test]
    fn chunked_delivery() {
        let mut d = KeyDecoder::new();
        assert!(d.feed(b"\x1b[").is_empty() || d.feed(b"").is_empty());
        let keys = d.feed(b"5~");
        assert_eq!(keys[0], Key::plain(KeyCode::PageUp));
    }

    #[test]
    fn mixed_stream() {
        let keys = decode(b"a\x01\x1b[A\x1bj\xE7\x95\x8C");
        assert_eq!(
            keys,
            vec![
                Key::char('a'),
                Key::ctrl('a'),
                Key::plain(KeyCode::Up),
                Key::alt('j'),
                Key::char('界')
            ]
        );
    }
}
