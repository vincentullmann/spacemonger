//! Content areas inside folder boxes.

use crate::core::geometry::{Insets, Rect};

/// A folder box's 3px frame and 12px title bar.
const FOLDER_FRAME: Insets = Insets::new(3.0, 12.0, 3.0, 3.0);

/// Content area of a folder box: inside its frame and title bar.
pub fn content(b: Rect) -> Rect {
    b - FOLDER_FRAME
}

/// Content area of the scan root when its box (the camera) is `cam`.
pub fn root_content(cam: Rect) -> Rect {
    cam - Insets::new(0.0, 0.0, 1.0, 1.0)
}
