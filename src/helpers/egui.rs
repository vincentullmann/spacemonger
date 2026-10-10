//! egui conveniences.

use eframe::egui::{Align, Align2, Pos2, Rect, Vec2};

/// Round a point to the nearest physical pixel.
pub fn snap(p: Pos2, ppp: f32) -> Pos2 {
    Pos2::new((p.x * ppp).round() / ppp, (p.y * ppp).round() / ppp)
}

/// Where to put a popup next to the pointer at `p` inside `screen`: below-right, flipping to
/// the other side near the right / bottom edges (assuming a popup of up to `size`).
pub fn popup_pivot(p: Pos2, screen: Rect, size: Vec2) -> (Align2, Vec2) {
    let pivot = match (p.x > screen.max.x - size.x, p.y > screen.max.y - size.y) {
        (false, false) => Align2::LEFT_TOP,
        (true, false) => Align2::RIGHT_TOP,
        (false, true) => Align2::LEFT_BOTTOM,
        (true, true) => Align2::RIGHT_BOTTOM,
    };
    let off = Vec2::new(
        if pivot.x() == Align::Min { 14.0 } else { -6.0 },
        if pivot.y() == Align::Min { 18.0 } else { -6.0 },
    );
    (pivot, off)
}
