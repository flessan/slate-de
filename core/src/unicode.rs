//! Minimal Unicode display-width support.
//!
//! Full `wcwidth` needs large tables; for a terminal dashboard a pragmatic
//! subset (CJK wide ranges, emoji planes, zero-width combining marks and
//! control characters) covers essentially every real-world string Slate
//! renders. Width rules are isolated here so a generated table can replace
//! them later without touching render code.

/// Display width of a single character in terminal cells.
pub fn char_width(c: char) -> usize {
    let cp = c as u32;
    if cp < 0x20 || (0x7F..0xA0).contains(&cp) {
        return 0; // control characters occupy no cell
    }
    if is_zero_width(cp) {
        return 0;
    }
    if is_wide(cp) {
        return 2;
    }
    1
}

fn is_zero_width(cp: u32) -> bool {
    matches!(
        cp,
        0x0300..=0x036F // combining diacriticals
        | 0x200B..=0x200F // zero-width space/formatters
        | 0x202A..=0x202E // bidi controls
        | 0xFE00..=0xFE0F // variation selectors
        | 0xFE20..=0xFE2F
    )
}

fn is_wide(cp: u32) -> bool {
    matches!(
        cp,
        0x1100..=0x115F        // Hangul Jamo
        | 0x2E80..=0x303E      // CJK radicals … ideographic punctuation
        | 0x3041..=0x33FF      // Hiragana … CJK compatibility
        | 0x3400..=0x4DBF      // CJK ext A
        | 0x4E00..=0x9FFF      // CJK unified
        | 0xA000..=0xA4CF      // Yi
        | 0xAC00..=0xD7A3      // Hangul syllables
        | 0xF900..=0xFAFF      // CJK compatibility ideographs
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60      // fullwidth forms
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF    // emoji & symbols planes
        | 0x20000..=0x2FFFD
        | 0x30000..=0x3FFFD
    )
}

/// Display width of a string.
pub fn str_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Truncate `s` to at most `max_width` display cells.
pub fn truncate(s: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = char_width(c);
        if w + cw > max_width {
            break;
        }
        w += cw;
        out.push(c);
    }
    out
}

/// Truncate with an ellipsis marker if truncation occurred.
pub fn ellipsize(s: &str, max_width: usize) -> String {
    if str_width(s) <= max_width {
        return s.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let mut out = truncate(s, max_width.saturating_sub(1));
    out.push('…');
    out
}

/// Pad on the right with spaces to reach `width` cells (no-op if longer).
pub fn pad_right(s: &str, width: usize) -> String {
    let w = str_width(s);
    if w >= width {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + (width - w));
    out.push_str(s);
    out.push_str(&" ".repeat(width - w));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('ä'), 1);
        assert_eq!(char_width('界'), 2);
        assert_eq!(char_width('한'), 2);
        assert_eq!(char_width('🦀'), 2);
        assert_eq!(char_width('\u{0301}'), 0); // combining acute
        assert_eq!(char_width('\n'), 0);
    }

    #[test]
    fn string_width_and_truncate() {
        assert_eq!(str_width("a界b"), 4);
        assert_eq!(truncate("a界bcd", 3).as_str(), "a界");
        assert_eq!(truncate("a界bcd", 2).as_str(), "a");
    }

    #[test]
    fn ellipsize_behavior() {
        assert_eq!(ellipsize("hello", 10), "hello");
        assert_eq!(ellipsize("hello world", 5), "hell…");
        assert_eq!(ellipsize("hi", 0), "");
    }

    #[test]
    fn padding() {
        assert_eq!(pad_right("ab", 5), "ab   ");
        assert_eq!(pad_right("abcdef", 3), "abcdef");
        assert_eq!(pad_right("界", 4), "界  ");
    }
}
