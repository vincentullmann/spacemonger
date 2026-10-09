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
    let eps = 1e-6 * (from.w + from.h);
    let overlap = |a0: f64, a1: f64, b0: f64, b1: f64| a1.min(b1) - a0.max(b0);
    let mut best: Option<(f64, f64, usize)> = None;
    for &(i, c) in boxes {
        let (gap, across, pos) = match d {
            Dir::Right => (c.x - (from.x + from.w), overlap(from.y, from.y + from.h, c.y, c.y + c.h), c.y),
            Dir::Left => (from.x - (c.x + c.w), overlap(from.y, from.y + from.h, c.y, c.y + c.h), c.y),
            Dir::Down => (c.y - (from.y + from.h), overlap(from.x, from.x + from.w, c.x, c.x + c.w), c.x),
            Dir::Up => (from.y - (c.y + c.h), overlap(from.x, from.x + from.w, c.x, c.x + c.w), c.x),
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
