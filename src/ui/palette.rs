//! Colours and drawing options for the treemap and path bar. The "Classic" scheme is the
//! original ColorService "Rainbow" palette; others are sampled from colorgrad presets.

use crate::ui::settings::{Font, Settings, Tiles};
use colorgrad::Gradient;
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

const fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Rainbow by depth. The classic scheme uses the first 8; more colours continue into the
/// original palette's bright / dark rows.
const BOX_LIGHT: [Color32; 24] = [
    c(0xFF, 0x7F, 0x7F),
    c(0xFF, 0xBF, 0x7F),
    c(0xFF, 0xFF, 0x00),
    c(0x7F, 0xFF, 0x7F),
    c(0x7F, 0xFF, 0xFF),
    c(0xBF, 0xBF, 0xFF),
    c(0xBF, 0xBF, 0xBF),
    c(0xFF, 0x7F, 0xFF),
    c(0xFF, 0xBF, 0xBF),
    c(0xFF, 0xDF, 0xBF),
    c(0xFF, 0xFF, 0xBF),
    c(0xBF, 0xFF, 0xBF),
    c(0xDF, 0xFF, 0xFF),
    c(0xDF, 0xDF, 0xFF),
    c(0xDF, 0xDF, 0xDF),
    c(0xFF, 0xBF, 0xFF),
    c(0xBF, 0x7F, 0x7F),
    c(0xBF, 0x9F, 0x5F),
    c(0xBF, 0xBF, 0x3F),
    c(0x7F, 0xBF, 0x7F),
    c(0x7F, 0xBF, 0xBF),
    c(0x9F, 0x9F, 0xFF),
    c(0x9F, 0x9F, 0x9F),
    c(0xBF, 0x7F, 0xBF),
];

const BOX_DARK: [Color32; 24] = [
    c(0x8F, 0x3F, 0x3F),
    c(0x90, 0x60, 0x32),
    c(0x8A, 0x82, 0x28),
    c(0x3F, 0x78, 0x3F),
    c(0x35, 0x78, 0x78),
    c(0x55, 0x55, 0x9A),
    c(0x62, 0x62, 0x62),
    c(0x82, 0x42, 0x82),
    c(0x75, 0x48, 0x48),
    c(0x78, 0x58, 0x3F),
    c(0x78, 0x72, 0x40),
    c(0x48, 0x70, 0x48),
    c(0x42, 0x70, 0x70),
    c(0x5A, 0x5A, 0x85),
    c(0x50, 0x50, 0x50),
    c(0x70, 0x48, 0x70),
    c(0x55, 0x28, 0x28),
    c(0x55, 0x3A, 0x20),
    c(0x55, 0x50, 0x18),
    c(0x28, 0x50, 0x28),
    c(0x25, 0x50, 0x50),
    c(0x38, 0x38, 0x70),
    c(0x38, 0x38, 0x38),
    c(0x50, 0x28, 0x50),
];

/// Where the depth colours come from.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Scheme {
    /// The original SpaceMonger rainbow (with its own dark-mode table).
    #[default]
    Classic,
    Turbo,
    Sinebow,
    Rainbow,
    Spectral,
    Viridis,
    Plasma,
    Warm,
    Cool,
    /// Hand-picked colours.
    Custom,
}

impl Scheme {
    /// How many colours the scheme starts with: the stops it's defined by, or 8 for the
    /// continuous gradients. `None` for `Custom`.
    pub fn native_count(self) -> Option<usize> {
        match self {
            Scheme::Classic => Some(8),
            Scheme::Viridis => Some(9),
            Scheme::Spectral | Scheme::Plasma => Some(11),
            Scheme::Turbo | Scheme::Sinebow | Scheme::Rainbow | Scheme::Warm | Scheme::Cool => {
                Some(8)
            }
            Scheme::Custom => None,
        }
    }

    /// `n` colours for this scheme, or `None` for `Custom`.
    pub fn colors(self, n: usize) -> Option<Vec<Color32>> {
        use colorgrad::preset;
        let n = n.max(1);
        Some(match self {
            Scheme::Classic => classic(&BOX_LIGHT, n),
            Scheme::Turbo => sample(preset::turbo(), n, false),
            Scheme::Sinebow => sample(preset::sinebow(), n, true),
            Scheme::Rainbow => sample(preset::rainbow(), n, true),
            Scheme::Spectral => sample(preset::spectral(), n, false),
            Scheme::Viridis => sample(preset::viridis(), n, false),
            Scheme::Plasma => sample(preset::plasma(), n, false),
            Scheme::Warm => sample(preset::warm(), n, false),
            Scheme::Cool => sample(preset::cool(), n, false),
            Scheme::Custom => return None,
        })
    }
}

/// `n` evenly spaced colours along a gradient. Cyclic gradients would repeat their first
/// colour at the end, so they stop one step short.
fn sample(g: impl Gradient, n: usize, cyclic: bool) -> Vec<Color32> {
    // Linear ones skip their very dark / very light ends.
    let (t0, t1) = if cyclic {
        (0.0, 1.0 - 1.0 / n as f32)
    } else {
        (0.1, 0.9)
    };
    let step = if n > 1 {
        (t1 - t0) / (n - 1) as f32
    } else {
        0.0
    };
    (0..n)
        .map(|i| {
            let [r, g2, b, _] = g.at(t0 + i as f32 * step).to_rgba8();
            Color32::from_rgb(r, g2, b)
        })
        .collect()
}

/// First `n` entries of a classic table: the 8 main colours, then the bright and dark rows.
fn classic(table: &[Color32; 24], n: usize) -> Vec<Color32> {
    (0..n).map(|i| table[i % table.len()]).collect()
}

/// Everything the painters need to know about how things look.
#[derive(Clone)]
pub struct Palette {
    pub background: Color32,
    pub text: Color32,
    /// Thin outline drawn inside every box.
    pub border: Color32,
    boxes: Vec<Color32>,
    /// Draw the thin outline (hover and selection outlines are always drawn).
    pub borders: bool,
    /// Space between neighbouring boxes, in points.
    pub gap: f32,
    /// How far a hovered box blends towards white (0..=1).
    pub hover: f32,
    /// Drop shadow behind labels.
    pub shadow: bool,
    /// Size and date lines in file boxes.
    pub details: bool,
    /// Label font size relative to the original 10 pt (spaces the label lines).
    pub text_scale: f32,
}

impl Palette {
    pub fn new(dark: bool, s: &Settings) -> Self {
        let Tiles {
            scheme,
            ref colors,
            borders,
            gap,
            hover,
        } = s.tiles;
        let Font { size, shadow, .. } = s.font;
        // Only the classic scheme has dark-mode colours of its own.
        let boxes = match (dark, scheme) {
            (true, Scheme::Classic) => classic(&BOX_DARK, colors.len()),
            _ => colors.clone(),
        };
        let (background, text, border) = if dark {
            (
                c(0x16, 0x17, 0x18),
                c(0xD0, 0xD0, 0xC8),
                c(0x10, 0x10, 0x10),
            )
        } else {
            (c(0xEE, 0xEE, 0xEE), Color32::BLACK, c(0x55, 0x55, 0x55))
        };
        Self {
            background,
            text,
            border,
            boxes,
            borders,
            gap: gap as f32,
            hover: hover as f32 / 100.0,
            shadow,
            details: s.layout.file_details,
            text_scale: size / 10.0,
        }
    }

    /// Label colour on a box filled with `fill`: the theme's text colour, unless the fill is
    /// too close to it (custom colours), then the opposite.
    pub fn text_on(&self, fill: Color32) -> Color32 {
        let lum = fill.intensity();
        let dark_text = self.text.intensity() < 0.5;
        match (dark_text, lum) {
            (true, l) if l < 0.35 => c(0xEE, 0xEE, 0xEE),
            (false, l) if l > 0.65 => Color32::BLACK,
            _ => self.text,
        }
    }

    /// Fill colour for a nesting depth.
    pub fn depth(&self, depth: i32) -> Color32 {
        if self.boxes.is_empty() {
            return Color32::GRAY;
        }
        self.boxes[depth.rem_euclid(self.boxes.len() as i32) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemes_give_n_colours() {
        assert_eq!(Scheme::Classic.colors(8).unwrap(), BOX_LIGHT[..8].to_vec());
        assert_eq!(Scheme::Turbo.colors(5).unwrap().len(), 5);
        assert_eq!(Scheme::Sinebow.colors(1).unwrap().len(), 1);
        assert!(Scheme::Custom.colors(3).is_none());
        // Cyclic: last colour isn't the first one again.
        let s = Scheme::Sinebow.colors(6).unwrap();
        assert_ne!(s[0], s[5]);
    }
}
