//! Arrow-key neighbours between sibling boxes.

use crate::core::geometry::Rect;

/// Direction of an arrow-key move.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// The box next to `from` in direction `d`: among boxes entirely on that side and overlapping
/// it across the direction, the closest; on a tie (several smaller neighbours along one edge)
/// the top-most for left/right moves, the left-most for up/down.
pub fn neighbour(boxes: &[(usize, Rect)], from: Rect, d: Dir) -> Option<usize> {
    let eps = 1e-6 * (from.width() + from.height());
    let overlap = |a0: f64, a1: f64, b0: f64, b1: f64| a1.min(b1) - a0.max(b0);
    let mut best: Option<(f64, f64, usize)> = None;
    for &(i, c) in boxes {
        let (gap, across, pos) = match d {
            Dir::Right => (c.x0 - from.x1, overlap(from.y0, from.y1, c.y0, c.y1), c.y0),
            Dir::Left => (from.x0 - c.x1, overlap(from.y0, from.y1, c.y0, c.y1), c.y0),
            Dir::Down => (c.y0 - from.y1, overlap(from.x0, from.x1, c.x0, c.x1), c.x0),
            Dir::Up => (from.y0 - c.y1, overlap(from.x0, from.x1, c.x0, c.x1), c.x0),
        };
        if gap < -eps || across <= eps {
            continue;
        }
        let better = match best {
            None => true,
            Some((g, q, _)) => gap < g - eps || (gap <= g + eps && pos < q),
        };
        if better {
            best = Some((gap, pos, i));
        }
    }
    best.map(|b| b.2)
}
