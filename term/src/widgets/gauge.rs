//! Progress gauges (CPU, memory, battery…).

use slate_core::rect::Rect;
use slate_core::style::Style;

use crate::buffer::Buffer;

/// Gauge styling and glyphs.
pub struct GaugeSpec<'a> {
    /// 0.0-1.0
    pub ratio: f64,
    pub label: &'a str,
    pub filled_style: Style,
    pub empty_style: Style,
    pub label_style: Style,
    pub filled_char: char,
    pub empty_char: char,
}

/// Render a one-row gauge into the first row of `area`.
pub fn gauge(buf: &mut Buffer, area: Rect, spec: &GaugeSpec<'_>) {
    if area.w == 0 || area.h == 0 {
        return;
    }
    let y = area.y;
    let ratio = spec.ratio.clamp(0.0, 1.0);
    let filled = (f64::from(area.w) * ratio).round() as u16;
    for i in 0..area.w {
        let (ch, style) =
            if i < filled { (spec.filled_char, spec.filled_style) } else { (spec.empty_char, spec.empty_style) };
        buf.set_char(area.x + i, y, ch, style);
    }
    // Overlay the centered label.
    let lw = slate_core::unicode::str_width(spec.label) as u16;
    if lw <= area.w {
        let start = area.x + (area.w - lw) / 2;
        buf.write_str(start, y, spec.label, spec.label_style, lw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::style::Color;

    fn spec(ratio: f64, label: &str) -> GaugeSpec<'_> {
        GaugeSpec {
            ratio,
            label,
            filled_style: Style::new().fg(Color::Named(2)),
            empty_style: Style::new().fg(Color::Named(8)),
            label_style: Style::new().fg(Color::Named(7)),
            filled_char: '█',
            empty_char: '░',
        }
    }

    #[test]
    fn renders_ratio() {
        let mut buf = Buffer::new(10, 1);
        gauge(&mut buf, buf.area(), &spec(0.5, "50%"));
        assert_eq!(buf.line_string(0), "███50%░░░░");
        assert_eq!(buf.get(0, 0).fg, Color::Named(2));
        assert_eq!(buf.get(9, 0).fg, Color::Named(7)); // label overlaid
        assert_eq!(buf.get(7, 0).fg, Color::Named(7));
    }

    #[test]
    fn clamps_and_edges() {
        let mut buf = Buffer::new(4, 1);
        gauge(&mut buf, buf.area(), &spec(1.5, ""));
        assert_eq!(buf.line_string(0), "████");
        let mut buf2 = Buffer::new(4, 1);
        gauge(&mut buf2, buf2.area(), &spec(-1.0, ""));
        assert_eq!(buf2.line_string(0), "░░░░");
        // zero-size area: no panic
        let mut buf3 = Buffer::new(4, 1);
        gauge(&mut buf3, Rect::new(0, 0, 0, 0), &spec(0.5, "x"));
    }
}
