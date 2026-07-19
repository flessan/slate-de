//! Bordered boxes with optional titles — the frame around every pane.

use slate_core::rect::Rect;
use slate_core::style::Style;

use crate::buffer::Buffer;

/// Which sides of a block have borders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Borders(pub u8);

impl Borders {
    pub const NONE: Borders = Borders(0);
    pub const LEFT: Borders = Borders(1);
    pub const RIGHT: Borders = Borders(2);
    pub const TOP: Borders = Borders(4);
    pub const BOTTOM: Borders = Borders(8);
    pub const ALL: Borders = Borders(15);

    pub fn contains(self, side: Borders) -> bool {
        self.0 & side.0 == side.0
    }
}

/// Glyph family for borders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BorderKind {
    #[default]
    Rounded,
    Plain,
    Double,
    /// Pure ASCII for maximum compatibility (`+---+` / `|`).
    Ascii,
}

struct BorderGlyphs {
    tl: char,
    tr: char,
    bl: char,
    br: char,
    h: char,
    v: char,
}

fn glyphs(kind: BorderKind) -> BorderGlyphs {
    match kind {
        BorderKind::Rounded => {
            BorderGlyphs { tl: '╭', tr: '╮', bl: '╰', br: '╯', h: '─', v: '│' }
        }
        BorderKind::Plain => BorderGlyphs { tl: '┌', tr: '┐', bl: '└', br: '┘', h: '─', v: '│' },
        BorderKind::Double => {
            BorderGlyphs { tl: '╔', tr: '╗', bl: '╚', br: '╝', h: '═', v: '║' }
        }
        BorderKind::Ascii => BorderGlyphs { tl: '+', tr: '+', bl: '+', br: '+', h: '-', v: '|' },
    }
}

/// How to draw a [`block`].
pub struct BlockSpec<'a> {
    pub title: Option<&'a str>,
    pub kind: BorderKind,
    pub borders: Borders,
    pub border_style: Style,
    pub title_style: Style,
    /// Extra padding inside the border (cells).
    pub pad: u16,
}

impl<'a> BlockSpec<'a> {
    pub fn titled(title: &'a str) -> Self {
        BlockSpec {
            title: Some(title),
            kind: BorderKind::Rounded,
            borders: Borders::ALL,
            border_style: Style::default(),
            title_style: Style::default(),
            pad: 0,
        }
    }

    pub fn focused(mut self, yes: bool, focus_style: Style) -> Self {
        if yes {
            self.border_style = focus_style;
            self.title_style = focus_style;
        }
        self
    }
}

/// Draw the block and return the inner content rectangle.
pub fn block(buf: &mut Buffer, area: Rect, spec: &BlockSpec<'_>) -> Rect {
    if area.w < 2 || area.h < 2 {
        return area;
    }
    let g = glyphs(spec.kind);
    let s = spec.border_style;
    let (x0, y0) = (area.x, area.y);
    let (x1, y1) = (area.right() - 1, area.bottom() - 1);

    if spec.borders.contains(Borders::TOP) {
        for x in x0..=x1 {
            buf.set_char(x, y0, g.h, s);
        }
    }
    if spec.borders.contains(Borders::BOTTOM) {
        for x in x0..=x1 {
            buf.set_char(x, y1, g.h, s);
        }
    }
    if spec.borders.contains(Borders::LEFT) {
        for y in y0..=y1 {
            buf.set_char(x0, y, g.v, s);
        }
    }
    if spec.borders.contains(Borders::RIGHT) {
        for y in y0..=y1 {
            buf.set_char(x1, y, g.v, s);
        }
    }
    if spec.borders.contains(Borders::TOP) {
        if spec.borders.contains(Borders::LEFT) {
            buf.set_char(x0, y0, g.tl, s);
        }
        if spec.borders.contains(Borders::RIGHT) {
            buf.set_char(x1, y0, g.tr, s);
        }
        if let Some(title) = spec.title {
            let text = format!(" {title} ");
            buf.write_str(x0 + 2, y0, &text, spec.title_style, area.w.saturating_sub(4));
        }
    }
    if spec.borders.contains(Borders::BOTTOM) {
        if spec.borders.contains(Borders::LEFT) {
            buf.set_char(x0, y1, g.bl, s);
        }
        if spec.borders.contains(Borders::RIGHT) {
            buf.set_char(x1, y1, g.br, s);
        }
    }

    let dx = u16::from(spec.borders.contains(Borders::LEFT))
        + u16::from(spec.borders.contains(Borders::RIGHT))
        + 2 * spec.pad;
    let dy = u16::from(spec.borders.contains(Borders::TOP))
        + u16::from(spec.borders.contains(Borders::BOTTOM))
        + 2 * spec.pad;
    Rect::new(
        area.x + u16::from(spec.borders.contains(Borders::LEFT)) + spec.pad,
        area.y + u16::from(spec.borders.contains(Borders::TOP)) + spec.pad,
        area.w.saturating_sub(dx),
        area.h.saturating_sub(dy),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::style::Color;

    #[test]
    fn draws_box_with_title() {
        let mut buf = Buffer::new(12, 5);
        let spec = BlockSpec::titled("main");
        let inner = block(&mut buf, buf.area(), &spec);
        assert_eq!(buf.get(0, 0).ch, '╭');
        assert_eq!(buf.get(11, 0).ch, '╮');
        assert_eq!(buf.get(0, 4).ch, '╰');
        assert_eq!(buf.get(11, 4).ch, '╯');
        assert_eq!(buf.get(3, 0).ch, 'm');
        assert_eq!(inner, Rect::new(1, 1, 10, 3));
    }

    #[test]
    fn ascii_kind() {
        let mut buf = Buffer::new(6, 3);
        let spec = BlockSpec { kind: BorderKind::Ascii, ..BlockSpec::titled("x") };
        block(&mut buf, buf.area(), &spec);
        assert_eq!(buf.get(0, 0).ch, '+');
        assert_eq!(buf.get(0, 1).ch, '|');
    }

    #[test]
    fn focused_style_applies() {
        let mut buf = Buffer::new(6, 3);
        let focus = Style::new().fg(Color::Named(6));
        let spec = BlockSpec::titled("x").focused(true, focus);
        block(&mut buf, buf.area(), &spec);
        assert_eq!(buf.get(5, 0).fg, Color::Named(6));
    }

    #[test]
    fn tiny_areas_do_not_panic() {
        let mut buf = Buffer::new(4, 2);
        let inner = block(&mut buf, Rect::new(0, 0, 1, 1), &BlockSpec::titled("t"));
        assert!(inner.w <= 1);
    }

    #[test]
    fn borders_none_gives_full_area() {
        let mut buf = Buffer::new(4, 2);
        let spec = BlockSpec { borders: Borders::NONE, ..BlockSpec::titled("t") };
        let inner = block(&mut buf, buf.area(), &spec);
        assert_eq!(inner, buf.area());
    }
}
