//! Render a [`Buffer`] as an SVG image.
//!
//! Powers `slate snapshot` (README screenshots that never drift from the real
//! UI) and golden tests in CI.

use slate_core::style::{Attrs, Color};

use crate::buffer::Buffer;

/// A named palette used to instantiate terminal palette colors in SVG.
const ANSI16: [&str; 16] = [
    "#000000", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5",
    "#666666", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#ffffff",
];

/// Concrete hex for any [`Color`] (None = "use the default").
pub fn color_hex(c: Color) -> Option<String> {
    match c {
        Color::Default => None,
        Color::Named(n) => Some(ANSI16[usize::from(n.min(15))].to_string()),
        Color::Indexed(n) => Some(indexed_hex(n)),
        Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
    }
}

fn indexed_hex(n: u8) -> String {
    if n < 16 {
        return ANSI16[usize::from(n)].to_string();
    }
    if n < 232 {
        const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let v = n - 16;
        let r = LEVELS[usize::from(v / 36 % 6)];
        let g = LEVELS[usize::from(v / 6 % 6)];
        let b = LEVELS[usize::from(v % 6)];
        return format!("#{r:02x}{g:02x}{b:02x}");
    }
    let v = 8 + u16::from(n - 232) * 10;
    format!("#{v:02x}{v:02x}{v:02x}")
}

/// SVG rendering knobs.
pub struct SvgOptions {
    pub cell_w: u32,
    pub cell_h: u32,
    pub font_size: u32,
    pub padding: u32,
    /// Page background (= terminal default background).
    pub bg: String,
    /// Default text color (= terminal default foreground).
    pub fg: String,
    pub font_family: String,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions {
            cell_w: 9,
            cell_h: 19,
            font_size: 15,
            padding: 10,
            bg: "#101014".into(),
            fg: "#e6e6e6".into(),
            font_family: "'JetBrains Mono','Fira Code',ui-monospace,SFMono-Regular,Menlo,Consolas,monospace".into(),
        }
    }
}

impl SvgOptions {
    /// Scale the whole canvas (used for hi-DPI exports).
    pub fn scaled(mut self, factor: u32) -> Self {
        let factor = factor.max(1);
        self.cell_w *= factor;
        self.cell_h *= factor;
        self.font_size *= factor;
        self.padding *= factor;
        self
    }
}

fn escape_xml(s: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\'' => s.push_str("&apos;"),
            _ => s.push(ch),
        }
    }
}

/// Serialize `buf` to a standalone SVG document.
pub fn buffer_to_svg(buf: &Buffer, opts: &SvgOptions) -> String {
    let w = u32::from(buf.width());
    let h = u32::from(buf.height());
    let width = w * opts.cell_w + 2 * opts.padding;
    let height = h * opts.cell_h + 2 * opts.padding;

    let mut out = String::with_capacity(usize::try_from(width * height / 2).unwrap_or(8192));
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" font-family=\"{}\" font-size=\"{}\" >\n",
        opts.font_family, opts.font_size
    ));
    out.push_str(&format!(
        "<rect width=\"{width}\" height=\"{height}\" fill=\"{}\" rx=\"6\"/>\n",
        opts.bg
    ));

    for y in 0..buf.height() {
        let py = opts.padding + u32::from(y) * opts.cell_h;
        // 1) background runs
        let mut run_start = 0u16;
        let mut run_bg = buf.get(0, y).bg;
        let mut x = 1u16;
        while x <= buf.width() {
            let bg = if x < buf.width() { buf.get(x, y).bg } else { Color::Default };
            let flush = x == buf.width() || bg != run_bg;
            if flush {
                if let Some(hex) = color_hex(run_bg) {
                    let px = opts.padding + u32::from(run_start) * opts.cell_w;
                    let rw = u32::from(x - run_start) * opts.cell_w;
                    out.push_str(&format!(
                        "<rect x=\"{px}\" y=\"{py}\" width=\"{rw}\" height=\"{}\" fill=\"{hex}\"/>\n",
                        opts.cell_h
                    ));
                }
                run_start = x;
                run_bg = bg;
            }
            x += 1;
        }
        // 2) text runs (same fg + attrs)
        let mut run_start = 0u16;
        let mut run_key = text_key(buf.get(0, y));
        let mut x = 1u16;
        while x <= buf.width() {
            let key = if x < buf.width() { text_key(buf.get(x, y)) } else { (None, Attrs::NONE, true) };
            let flush = x == buf.width() || key != run_key;
            if flush {
                emit_text_run(&mut out, buf, y, run_start, x, &run_key, opts);
                run_start = x;
                run_key = key;
            }
            x += 1;
        }
    }
    out.push_str("</svg>\n");
    out
}

/// (fg, attrs, is_continuation) — run boundaries in the text pass.
fn text_key(cell: &crate::cell::Cell) -> (Option<String>, Attrs, bool) {
    (color_hex(cell.fg), cell.attrs, cell.cont)
}

fn emit_text_run(
    out: &mut String,
    buf: &Buffer,
    y: u16,
    start: u16,
    end: u16,
    key: &(Option<String>, Attrs, bool),
    opts: &SvgOptions,
) {
    let (fg, attrs, _) = key;
    let mut text = String::new();
    for x in start..end {
        let cell = buf.get(x, y);
        if !cell.cont {
            text.push(cell.ch);
        }
    }
    let visible = text.trim_end();
    if visible.is_empty() {
        return; // whitespace rows are covered by background rects
    }
    let px = opts.padding + u32::from(start) * opts.cell_w;
    let baseline = opts.padding + u32::from(y) * opts.cell_h + opts.cell_h.saturating_sub(5);
    let fill = fg.as_deref().unwrap_or(opts.fg.as_str());
    let mut decorations = String::new();
    if attrs.contains(Attrs::BOLD) {
        decorations.push_str(" font-weight=\"bold\"");
    }
    if attrs.contains(Attrs::ITALIC) {
        decorations.push_str(" font-style=\"italic\"");
    }
    if attrs.contains(Attrs::UNDERLINE) {
        decorations.push_str(" text-decoration=\"underline\"");
    }
    if attrs.contains(Attrs::DIM) {
        decorations.push_str(" opacity=\"0.7\"");
    }
    out.push_str(&format!(
        "<text x=\"{px}\" y=\"{baseline}\" fill=\"{fill}\" xml:space=\"preserve\"{decorations}>"
    ));
    escape_xml(out, &text);
    out.push_str("</text>\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::style::{Color, Line, Span, Style};

    #[test]
    fn indexed_palette_edges() {
        assert_eq!(color_hex(Color::Indexed(0)).unwrap(), "#000000");
        assert_eq!(color_hex(Color::Indexed(196)).unwrap(), "#ff0000");
        assert_eq!(color_hex(Color::Indexed(21)).unwrap(), "#0000ff");
        assert_eq!(color_hex(Color::Indexed(232)).unwrap(), "#080808");
    }

    #[test]
    fn svg_contains_text_and_colors() {
        let mut buf = Buffer::new(10, 2);
        let line = Line::new(vec![
            Span::plain("hi "),
            Span::styled("<ok>", Style::new().fg(Color::Named(2)).bold()),
        ]);
        buf.write_line(0, 0, &line, 10);
        let svg = buffer_to_svg(&buf, &SvgOptions::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>\n"));
        assert!(svg.contains("&lt;ok&gt;")); // XML-escaped
        assert!(svg.contains("#0dbc79")); // green fg
        assert!(svg.contains("font-weight=\"bold\""));
        assert!(svg.contains(&SvgOptions::default().bg));
    }

    #[test]
    fn wide_char_does_not_duplicate() {
        let mut buf = Buffer::new(6, 1);
        buf.write_str(0, 0, "界a", Style::default(), 6);
        let svg = buffer_to_svg(&buf, &SvgOptions::default());
        assert!(svg.contains("界a") || svg.contains("界") && svg.contains(">a<"));
    }

    #[test]
    fn empty_buffer_is_valid_svg() {
        let svg = buffer_to_svg(&Buffer::new(4, 2), &SvgOptions::default());
        assert!(svg.contains("<rect"));
        assert!(!svg.contains("<text"));
    }
}
