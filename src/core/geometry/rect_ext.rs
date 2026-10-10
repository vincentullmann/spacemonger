//! Extra rectangle helpers.

use kurbo::Rect;

pub trait RectExt {
    /// Area, counting an inverted (negative-size) rectangle as empty.
    fn clamped_area(&self) -> f64;
    /// Does `self` contain `o`, with half a point of slack?
    fn covers(&self, o: &Rect) -> bool;
}

impl RectExt for Rect {
    fn clamped_area(&self) -> f64 {
        self.width().max(0.0) * self.height().max(0.0)
    }

    fn covers(&self, o: &Rect) -> bool {
        self.inflate(0.5, 0.5).contains_rect(*o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(Rect::new(0.0, 0.0, -2.0, 3.0).clamped_area(), 0.0);
        assert_eq!(Rect::new(0.0, 0.0, 2.0, 3.0).clamped_area(), 6.0);
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.covers(&Rect::new(-0.4, 0.0, 10.4, 10.0)));
        assert!(!r.covers(&Rect::new(-0.6, 0.0, 10.0, 10.0)));
    }
}
