//! Per-folder box reshaping (used to make a zoomed-in folder fill the window).

use crate::core::geometry::{Rect, Size, Vec2};

/// Reshapes the box of the entry at `path`: scaled per axis by `a` about its centre, with the
/// centre moved by `d` (in units of the natural box size). Both are relative to the natural
/// box, so the reshape zooms and pans with the camera.
#[derive(Clone, Debug, PartialEq)]
pub struct Reshape {
    pub path: Vec<usize>,
    pub a: Vec2,
    pub d: Vec2,
}

impl Reshape {
    pub fn apply(&self, n: Rect) -> Rect {
        let size = Size::new(n.width() * self.a.x, n.height() * self.a.y);
        let c = n.origin() + Vec2::new(n.width() * (0.5 + self.d.x), n.height() * (0.5 + self.d.y));
        Rect::from_center_size(c, size)
    }

    /// The reshape that turns natural box `n` into box `t`.
    pub fn between(path: Vec<usize>, n: Rect, t: Rect) -> Self {
        let (nw, nh) = (n.width(), n.height());
        let dc = t.center() - n.center();
        Reshape {
            path,
            a: Vec2::new(t.width() / nw, t.height() / nh),
            d: Vec2::new(dc.x / nw, dc.y / nh),
        }
    }

    /// This reshape at strength `b` (0 = natural box, 1 = full reshape).
    pub fn faded(&self, b: f64) -> Self {
        Reshape {
            path: self.path.clone(),
            a: Vec2::new(self.a.x.powf(b), self.a.y.powf(b)),
            d: self.d * b,
        }
    }
}
