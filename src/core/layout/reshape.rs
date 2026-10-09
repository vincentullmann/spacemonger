//! Per-folder box reshaping (used to make a zoomed-in folder fill the window).

use crate::core::geometry::Rect;

/// Reshapes the box of the entry at `path`: scaled per axis by `a` about its centre, with the
/// centre moved by `d` (in units of the natural box size). Both are relative to the natural
/// box, so the override zooms and pans with the camera.
#[derive(Clone, Debug, PartialEq)]
pub struct Reshape {
    pub path: Vec<usize>,
    pub a: (f64, f64),
    pub d: (f64, f64),
}

impl Reshape {
    pub fn apply(&self, n: Rect) -> Rect {
        let (w, h) = (n.w * self.a.0, n.h * self.a.1);
        let cx = n.x + n.w * (0.5 + self.d.0);
        let cy = n.y + n.h * (0.5 + self.d.1);
        Rect::new(cx - w / 2.0, cy - h / 2.0, w, h)
    }

    /// The override that turns natural box `n` into box `t`.
    pub fn between(path: Vec<usize>, n: Rect, t: Rect) -> Self {
        let (nc, tc) = (n.center(), t.center());
        Reshape { path, a: (t.w / n.w, t.h / n.h), d: ((tc.0 - nc.0) / n.w, (tc.1 - nc.1) / n.h) }
    }
}

impl Reshape {
    /// This reshape at strength `b` (0 = natural box, 1 = full reshape).
    pub fn faded(&self, b: f64) -> Self {
        Reshape { path: self.path.clone(), a: (self.a.0.powf(b), self.a.1.powf(b)), d: (self.d.0 * b, self.d.1 * b) }
    }
}
