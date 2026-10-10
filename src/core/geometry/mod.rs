//! Geometry: kurbo's f64 types (view coordinates, so deep zooms keep sub-pixel precision),
//! plus a couple of helpers.

mod rect_ext;

pub use kurbo::{Insets, Point, Rect, Size, TranslateScale, Vec2};
pub use rect_ext::RectExt;
