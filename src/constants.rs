//! App-wide constants.

use std::time::Duration;

pub const APP_NAME: &str = "SpaceMonger One";

/// Application id: window class / Wayland app id, and eframe's storage name.
pub const APP_ID: &str = "spacemonger";

/// Size of icon glyphs on buttons (title bar, dialog ×).
pub const ICON_SIZE: f32 = 16.0;

/// Zoom factor per point of wheel scroll at the default zoom speed (a mouse notch is ~50
/// points, about 1.2x).
pub const WHEEL_ZOOM: f64 = 0.004;

/// Deepest zoom, as a multiple of the view size.
pub const MAX_ZOOM: f64 = 1e8;

/// A resize step within this long of the previous one keeps the same anchor.
pub const RESIZE_SETTLE: Duration = Duration::from_millis(400);

/// Resize anchor: deepest folder at the window centre covering at least this share of it.
pub const ANCHOR_MIN_SHARE: f64 = 0.25;

/// How often a running scan is snapshotted for the treemap.
pub const LIVE_SCAN_INTERVAL: Duration = Duration::from_millis(100);

/// Path bar height at the default path-bar font size.
pub const BAR_H: i32 = 18;

/// Range of the UI zoom setting, in percent.
pub const UI_ZOOM: (u32, u32) = (50, 200);
