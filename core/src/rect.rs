//! Terminal geometry: rectangles and weight-based splitting.

/// Axis along which a region is split.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Children are placed side by side (columns). Produced by `|` in the DSL.
    Horizontal,
    /// Children are stacked on top of each other (rows). Produced by `/`.
    Vertical,
}

/// A rectangle in terminal cells. Origin is the top-left corner of the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub const ZERO: Rect = Rect { x: 0, y: 0, w: 0, h: 0 };

    pub const fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub fn right(&self) -> u16 {
        self.x.saturating_add(self.w)
    }

    pub fn bottom(&self) -> u16 {
        self.y.saturating_add(self.h)
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Shrink by `dx`/`dy` on every side (saturating).
    pub fn inset(&self, dx: u16, dy: u16) -> Rect {
        Rect {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
            w: self.w.saturating_sub(dx.saturating_mul(2)),
            h: self.h.saturating_sub(dy.saturating_mul(2)),
        }
    }
}

/// Distribute `total` cells proportionally across `weights`, leaving `gap`
/// cells between consecutive parts. Every part gets at least one weight unit;
/// the result sums to `total - gap * (n - 1)` (or to zeros when `total` is
/// too small).
pub fn split_sizes(weights: &[u16], total: u16, gap: u16) -> Vec<u16> {
    let n = weights.len();
    if n == 0 {
        return Vec::new();
    }
    let gaps = gap.saturating_mul((n as u16).saturating_sub(1));
    let avail = total.saturating_sub(gaps);
    if avail == 0 {
        return vec![0; n];
    }
    let norm: Vec<u64> = weights.iter().map(|&w| u64::from(w).max(1)).collect();
    let sum: u64 = norm.iter().sum();
    let exact: Vec<(u64, u64)> = norm
        .iter()
        .map(|&w| {
            let e = u64::from(avail) * w;
            (e / sum, e % sum)
        })
        .collect();
    let mut sizes: Vec<u16> = exact.iter().map(|&(b, _)| b as u16).collect();
    // Distribute the rounding leftover by largest remainder (stable).
    let used: u64 = sizes.iter().map(|&s| u64::from(s)).sum();
    let mut leftover = u64::from(avail) - used;
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| exact[b].1.cmp(&exact[a].1).then(a.cmp(&b)));
    let mut i = 0;
    while leftover > 0 {
        let idx = order[i % n];
        sizes[idx] = sizes[idx].saturating_add(1);
        leftover -= 1;
        i += 1;
    }
    sizes
}

/// Split `area` along `dir` into sub-rectangles sized by `weights`.
pub fn split_rects(area: Rect, dir: Direction, weights: &[u16], gap: u16) -> Vec<Rect> {
    match dir {
        Direction::Horizontal => {
            let mut x = area.x;
            split_sizes(weights, area.w, gap)
                .into_iter()
                .map(|w| {
                    let r = Rect::new(x, area.y, w, area.h);
                    x = x.saturating_add(w).saturating_add(gap);
                    r
                })
                .collect()
        }
        Direction::Vertical => {
            let mut y = area.y;
            split_sizes(weights, area.h, gap)
                .into_iter()
                .map(|h| {
                    let r = Rect::new(area.x, y, area.w, h);
                    y = y.saturating_add(h).saturating_add(gap);
                    r
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_split_evenly() {
        assert_eq!(split_sizes(&[1, 1], 20, 0), vec![10, 10]);
        assert_eq!(split_sizes(&[3, 1], 20, 0), vec![15, 5]);
        assert_eq!(split_sizes(&[1, 1, 1], 30, 0), vec![10, 10, 10]);
    }

    #[test]
    fn sizes_respect_gaps() {
        // 20 total, gap 1 between two parts → 19 available.
        let sizes = split_sizes(&[1, 1], 20, 1);
        assert_eq!(sizes.iter().sum::<u16>(), 19);
        assert_eq!(sizes, vec![10, 9]); // larger remainder goes first
    }

    #[test]
    fn sizes_rounding_is_stable() {
        let s = split_sizes(&[1, 1, 1], 10, 0); // 3.33 each
        assert_eq!(s.iter().sum::<u16>(), 10);
        assert_eq!(s, vec![4, 3, 3]);
    }

    #[test]
    fn sizes_degenerate() {
        assert_eq!(split_sizes(&[], 10, 0), Vec::<u16>::new());
        assert_eq!(split_sizes(&[1, 1], 1, 1), vec![0, 0]);
        assert_eq!(split_sizes(&[1], 7, 0), vec![7]);
    }

    #[test]
    fn rects_flow_correctly() {
        let area = Rect::new(0, 0, 21, 10);
        let cols = split_rects(area, Direction::Horizontal, &[1, 1], 1);
        assert_eq!(cols, vec![Rect::new(0, 0, 10, 10), Rect::new(11, 0, 10, 10)]);

        let rows = split_rects(area, Direction::Vertical, &[1, 3], 0);
        assert_eq!(rows[0], Rect::new(0, 0, 21, 2));
        assert_eq!(rows[1], Rect::new(0, 2, 21, 8));
    }

    #[test]
    fn rect_helpers() {
        let r = Rect::new(2, 3, 10, 5);
        assert_eq!(r.right(), 12);
        assert_eq!(r.bottom(), 8);
        assert!(r.contains(11, 7));
        assert!(!r.contains(12, 7));
        assert_eq!(r.inset(1, 1), Rect::new(3, 4, 8, 3));
        assert!(!Rect::ZERO.contains(0, 0));
    }
}
