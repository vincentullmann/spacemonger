//! Layout settings (density, split bias, free space).

/// Minimum (w, h) for a labelled box, by density (-3..=3).
const MIN_SIZES: [(f64, f64); 7] = [
    (96.0, 64.0),
    (64.0, 48.0),
    (48.0, 32.0),
    (32.0, 24.0),
    (24.0, 16.0),
    (16.0, 12.0),
    (8.0, 6.0),
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LayoutParams {
    pub density: i32,
    /// -20 (prefer vertical splits) .. +20 (prefer horizontal splits).
    pub bias: i32,
    pub show_free: bool,
    /// Leave out entries named `.something` (matches [`crate::core::model::Tree::dotfiles_hidden`]).
    pub hide_dotfiles: bool,
    /// Height of a folder's title bar (grows with the label font).
    pub title_h: f64,
}

impl Default for LayoutParams {
    fn default() -> Self {
        Self {
            density: 0,
            bias: 0,
            show_free: true,
            hide_dotfiles: false,
            title_h: super::content::TITLE_H,
        }
    }
}

impl LayoutParams {
    /// Minimum (w, h) a box needs to carry a label (and show a folder's contents).
    pub fn label_min(&self) -> (f64, f64) {
        MIN_SIZES[(self.density.clamp(-3, 3) + 3) as usize]
    }
}
