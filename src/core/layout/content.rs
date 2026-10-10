//! Content areas inside folder boxes.

use crate::core::geometry::{Insets, Rect};

/// Width of a folder box's frame (left, right, bottom).
pub const FOLDER_FRAME: f64 = 3.0;
/// Default height of a folder box's title bar (for 10px labels).
pub const TITLE_H: f64 = 12.0;

/// Content area of a folder box: inside its 3px frame and `title_h` title bar.
pub fn content(b: Rect, title_h: f64) -> Rect {
    b - Insets::new(FOLDER_FRAME, title_h, FOLDER_FRAME, FOLDER_FRAME)
}

/// Content area of the scan root when its box (the camera) is `cam`.
pub fn root_content(cam: Rect) -> Rect {
    cam - Insets::new(0.0, 0.0, 1.0, 1.0)
}
