//! Sparklines (CPU/history graphs) — eight-level block bars, newest value
//! on the right.

use slate_core::rect::Rect;
use slate_core::style::Style;

use crate::buffer::Buffer;

/// The eight bar levels (lowest → highest).
pub const SPARK_BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Sparkline inputs.
pub struct SparklineSpec<'a> {
    pub data: &'a [u64],
    /// Scale cap; defaults to `max(data)`.
    pub max: Option<u64>,
    pub style: Style,
}

/// Map a value to a bar glyph.
pub fn bar_for(value: u64, max: u64) -> char {
    if max == 0 || value == 0 {
        return SPARK_BARS[0];
    }
    let level = ((value.min(max) * 7) / max) as usize;
    SPARK_BARS[level.min(7)]
}

pub fn sparkline(buf: &mut Buffer, area: Rect, spec: &SparklineSpec<'_>) {
    if area.w == 0 || area.h == 0 || spec.data.is_empty() {
        return;
    }
    let take = usize::from(area.w).min(spec.data.len());
    let data = &spec.data[spec.data.len() - take..];
    let max = spec.max.unwrap_or_else(|| data.iter().copied().max().unwrap_or(0));
    let x0 = area.x + (area.w - take as u16);
    for (i, &v) in data.iter().enumerate() {
        buf.set_char(x0 + i as u16, area.y, bar_for(v, max), spec.style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_levels() {
        assert_eq!(bar_for(0, 100), '▁');
        assert_eq!(bar_for(100, 100), '█');
        assert_eq!(bar_for(50, 100), SPARK_BARS[3]); // 50*7/100 = 3 ▄
        assert_eq!(bar_for(5, 0), '▁');
    }

    #[test]
    fn right_aligned_render() {
        let data: Vec<u64> = (0..=20).collect();
        let mut buf = Buffer::new(10, 1);
        sparkline(&mut buf, buf.area(), &SparklineSpec { data: &data, max: None, style: Default::default() });
        let s = buf.line_string(0);
        assert_eq!(s.chars().count(), 10);
        assert_eq!(s.chars().last(), Some('█'));
        // First shown sample is 11 of max 20 → 11*7/20 = level 3.
        assert!(s.starts_with(SPARK_BARS[3]));
    }
}
