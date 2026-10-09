//! Axis-aligned rectangle in f64 view coordinates.

/// A rectangle in view coordinates (points). f64 so deep zooms keep sub-pixel precision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn area(&self) -> f64 {
        self.w.max(0.0) * self.h.max(0.0)
    }

    /// Does `self` contain `o` (with half a point of slack)?
    pub fn covers(&self, o: &Rect) -> bool {
        self.x <= o.x + 0.5 && self.y <= o.y + 0.5 && self.x + self.w >= o.x + o.w - 0.5 && self.y + self.h >= o.y + o.h - 0.5
    }
}
