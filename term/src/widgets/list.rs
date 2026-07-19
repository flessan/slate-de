//! Selectable lists (search results, notes, themes…).

use slate_core::rect::Rect;
use slate_core::style::{Line, Style};

use crate::buffer::Buffer;

/// List rendering inputs.
pub struct ListSpec<'a> {
    pub items: &'a [Line],
    /// Index (into `items`) of the highlighted row.
    pub selected: Option<usize>,
    /// First visible row (scroll offset).
    pub offset: usize,
    /// Applied to the selected row (background + attrs merged).
    pub highlight: Style,
}

/// Merge a highlight style over a span style: keep the span's foreground,
/// take the highlight's background and union attributes.
pub fn merge_highlight(highlight: Style, span: Style) -> Style {
    Style { fg: span.fg, bg: highlight.bg, attrs: highlight.attrs | span.attrs }
}

/// Render the list; returns the number of rows painted.
pub fn list(buf: &mut Buffer, area: Rect, spec: &ListSpec<'_>) -> usize {
    if area.w == 0 || area.h == 0 {
        return 0;
    }
    let mut rows = 0;
    for (i, line) in spec.items.iter().enumerate().skip(spec.offset) {
        if rows >= usize::from(area.h) {
            break;
        }
        let y = area.y + rows as u16;
        if Some(i) == spec.selected {
            buf.fill(Rect::new(area.x, y, area.w, 1), spec.highlight);
            let spans: Vec<_> = line
                .spans
                .iter()
                .map(|s| slate_core::style::Span::styled(&s.text, merge_highlight(spec.highlight, s.style)))
                .collect();
            buf.write_line(area.x, y, &Line::new(spans), area.w);
        } else {
            buf.write_line(area.x, y, line, area.w);
        }
        rows += 1;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::style::{Color, Span};

    #[test]
    fn renders_with_selection() {
        let items = vec![
            Line::plain("alpha"),
            Line::new(vec![Span::styled("beta", Style::new().fg(Color::Named(1)))]),
            Line::plain("gamma"),
        ];
        let hl = Style::new().bg(Color::Named(4)).bold();
        let mut buf = Buffer::new(10, 3);
        let spec = ListSpec { items: &items, selected: Some(1), offset: 0, highlight: hl };
        let rows = list(&mut buf, buf.area(), &spec);
        assert_eq!(rows, 3);
        assert_eq!(buf.line_string(1), "beta");
        assert_eq!(buf.get(0, 1).bg, Color::Named(4)); // highlight fills the row
        assert_eq!(buf.get(0, 1).fg, Color::Named(1)); // span fg preserved
        assert_eq!(buf.get(0, 0).bg, Color::Default);
    }

    #[test]
    fn respects_offset_and_height() {
        let items: Vec<Line> = (0..10).map(|i| Line::plain(format!("row{i}"))).collect();
        let mut buf = Buffer::new(10, 2);
        let spec = ListSpec { items: &items, selected: None, offset: 3, highlight: Style::default() };
        let rows = list(&mut buf, buf.area(), &spec);
        assert_eq!(rows, 2);
        assert_eq!(buf.line_string(0), "row3");
        assert_eq!(buf.line_string(1), "row4");
    }
}
