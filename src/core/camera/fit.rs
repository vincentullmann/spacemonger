//! A folder stretched to fill the window.

use crate::core::layout::Reshape;

/// A folder reshaped to fill the window. The camera itself always zooms evenly; only this
/// folder's box is stretched, by an amount that fades out as you zoom out from it.
#[derive(Clone)]
pub struct Fit {
    /// Full-strength reshape, relative to the folder's natural box.
    pub reshape: Reshape,
    /// Camera scale (root width / view width) at which the folder exactly fills the view.
    pub scale: f64,
}

impl Fit {
    /// Strength of the reshape at camera scale `s`: 1 at or above the fit scale, fading to 0
    /// by half of it (but always 0 when fully zoomed out).
    pub fn blend(&self, s: f64) -> f64 {
        let lo = (self.scale / 2.0).max(1.0);
        if self.scale - lo < 1e-9 {
            return if s >= self.scale { 1.0 } else { 0.0 };
        }
        ((s / lo).ln() / (self.scale / lo).ln()).clamp(0.0, 1.0)
    }
}
