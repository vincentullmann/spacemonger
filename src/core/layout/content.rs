//! Content areas inside folder boxes.

use crate::core::geometry::Rect;

/// Content area of a folder box: inside its 3px frame and 12px title bar.
pub fn content(b: Rect) -> Rect {
    Rect::new(b.x + 3.0, b.y + 12.0, b.w - 6.0, b.h - 15.0)
}

/// Content area of the scan root when its box (the camera) is `cam`.
pub fn root_content(cam: Rect) -> Rect {
    Rect::new(cam.x, cam.y, cam.w - 1.0, cam.h - 1.0)
}
