//! Multi-line styled text with scrolling and word wrapping.

use slate_core::rect::Rect;
use slate_core::style::{Line, Span, Style};
use slate_core::unicode;

use crate::buffer::Buffer;

/// Paragraph rendering options.
#[derive(Clone, Copy, Debug, Default)]
pub struct ParagraphOpts {
    /// First visible line (scroll offset).
    pub scroll: u16,
    /// Soft-wrap lines at the right edge.
    pub wrap: bool,
}

/// Render `lines` into `area`. Returns total display lines (after wrapping)
/// so callers can bound scroll offsets.
pub fn paragraph(buf: &mut Buffer, area: Rect, lines: &[Line], opts: ParagraphOpts) -> usize {
    if area.w == 0 || area.h == 0 {
        return 0;
    }
    let display: Vec<Line>;
    let all: &[Line] = if opts.wrap {
        display = lines.iter().flat_map(|l| wrap_line(l, area.w)).collect();
        &display
    } else {
        lines
    };
    let start = usize::from(opts.scroll).min(all.len());
    let end = (start + usize::from(area.h)).min(all.len());
    for (row, line) in all[start..end].iter().enumerate() {
        buf.write_line(area.x, area.y + row as u16, line, area.w);
    }
    all.len()
}

/// Word-wrap a styled line to `width` cells, preserving span styles.
/// Wraps at the last space when possible, hard-breaks long words.
pub fn wrap_line(line: &Line, width: u16) -> Vec<Line> {
    let width = usize::from(width);
    if width == 0 {
        return Vec::new();
    }
    // Flatten to (char, style).
    let mut flat: Vec<(char, Style)> = Vec::new();
    for span in &line.spans {
        for ch in span.text.chars() {
            flat.push((ch, span.style));
        }
    }
    let mut breaks: Vec<usize> = Vec::new(); // split points in `flat`
    let mut col = 0usize;
    let mut last_space: Option<usize> = None;
    let mut segment_start = 0usize;
    for (i, &(ch, _)) in flat.iter().enumerate() {
        if ch == ' ' {
            last_space = Some(i);
        }
        let cw = unicode::char_width(ch).max(1);
        if col + cw > width {
            match last_space {
                Some(sp) if sp >= segment_start && sp > segment_start => {
                    breaks.push(sp + 1); // start new line after the space
                    segment_start = sp + 1;
                }
                _ => {
                    breaks.push(i);
                    segment_start = i;
                }
            }
            col = flat[segment_start..=i]
                .iter()
                .map(|&(c, _)| unicode::char_width(c).max(1))
                .sum();
            last_space = None;
        } else {
            col += cw;
        }
    }
    if breaks.is_empty() {
        return vec![line.clone()];
    }
    let mut out = Vec::with_capacity(breaks.len() + 1);
    let mut prev = 0usize;
    for &b in breaks.iter().chain(std::iter::once(&flat.len())) {
        out.push(unflatten(&flat[prev..b]));
        prev = b;
    }
    out.retain(|l| !l.is_empty());
    out
}

/// Rebuild styled spans from a flat (char, style) slice, dropping leading
/// spaces introduced by word wrapping.
fn unflatten(flat: &[(char, Style)]) -> Line {
    let mut spans: Vec<Span> = Vec::new();
    let mut started = false;
    for &(ch, style) in flat {
        if !started && ch == ' ' {
            continue; // trim wrap indentation
        }
        started = true;
        match spans.last_mut() {
            Some(last) if last.style == style => last.text.push(ch),
            _ => spans.push(Span { text: ch.to_string(), style }),
        }
    }
    // Trim trailing spaces for a clean right edge.
    if let Some(last) = spans.last_mut() {
        let trimmed = last.text.trim_end().to_string();
        last.text = trimmed;
    }
    Line { spans }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::style::Color;

    #[test]
    fn no_wrap_short_text() {
        let mut buf = Buffer::new(20, 2);
        let lines = vec![Line::plain("hello"), Line::plain("world")];
        let total = paragraph(&mut buf, buf.area(), &lines, ParagraphOpts::default());
        assert_eq!(total, 2);
        assert_eq!(buf.line_string(0), "hello");
        assert_eq!(buf.line_string(1), "world");
    }

    #[test]
    fn clips_long_lines_without_wrap() {
        let mut buf = Buffer::new(5, 1);
        let lines = vec![Line::plain("abcdefghij")];
        paragraph(&mut buf, buf.area(), &lines, ParagraphOpts::default());
        assert_eq!(buf.line_string(0), "abcde");
    }

    #[test]
    fn scrolling_skips_lines() {
        let mut buf = Buffer::new(10, 1);
        let lines = vec![Line::plain("one"), Line::plain("two"), Line::plain("three")];
        let total =
            paragraph(&mut buf, buf.area(), &lines, ParagraphOpts { scroll: 2, wrap: false });
        assert_eq!(total, 3);
        assert_eq!(buf.line_string(0), "three");
    }

    #[test]
    fn word_wraps_at_spaces() {
        let l = Line::plain("the quick brown fox jumps");
        let wrapped = wrap_line(&l, 10);
        let texts: Vec<String> = wrapped.iter().map(Line::to_plain).collect();
        assert_eq!(texts, vec!["the quick", "brown fox", "jumps"]);
    }

    #[test]
    fn hard_breaks_long_words() {
        let l = Line::plain("abcdefghijklmnop");
        let wrapped = wrap_line(&l, 5);
        let texts: Vec<String> = wrapped.iter().map(Line::to_plain).collect();
        assert_eq!(texts, vec!["abcde", "fghij", "klmno", "p"]);
    }

    #[test]
    fn wrap_preserves_styles() {
        let l = Line::new(vec![
            Span::plain("aa "),
            Span::styled("bbbb", Style::new().fg(Color::Named(1))),
            Span::plain(" cc"),
        ]);
        let wrapped = wrap_line(&l, 5);
        let texts: Vec<String> = wrapped.iter().map(Line::to_plain).collect();
        assert_eq!(texts, vec!["aa", "bbbb", "cc"]);
        // The red span survived on the second output line.
        assert_eq!(wrapped[1].spans[0].style.fg, Color::Named(1));
    }

    #[test]
    fn wrap_mode_renders_wrapped() {
        let mut buf = Buffer::new(10, 3);
        let lines = vec![Line::plain("the quick brown fox jumps")];
        let total =
            paragraph(&mut buf, buf.area(), &lines, ParagraphOpts { scroll: 0, wrap: true });
        assert_eq!(total, 3);
        assert_eq!(buf.line_string(2), "jumps");
    }
}
