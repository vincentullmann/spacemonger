//! The tiles' colours and outlines, and keeping the colour scheme and list in step.

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use super::Line;
use crate::ui::palette::Scheme;

/// How the boxes look: colours, text colour, outlines, gap, hover.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Tiles {
    pub scheme: Scheme,
    /// One colour per nesting level, repeating.
    pub colors: Vec<Color32>,
    /// Label and folder title colour, used as is.
    pub text_color: Color32,
    /// Outline of every box; width 0 for none.
    pub border: Line,
    /// Outline of the box under the mouse.
    pub hover_border: Line,
    /// Fill and outline of selected boxes; `None` follows the theme.
    pub selection_color: Option<Color32>,
    pub gap: u8,
    pub hover: u8,
    /// The old "Borders" switch, read from saved settings only (see [`Settings::load`]).
    #[serde(rename = "borders", skip_serializing, deserialize_with = "some_bool")]
    pub legacy_borders: Option<bool>,
}

/// `after` is `before` with one colour taken out.
fn removed_one(before: &[Color32], after: &[Color32]) -> bool {
    after.len() + 1 == before.len()
        && (0..before.len()).any(|i| before[..i] == after[..i] && before[i + 1..] == after[i..])
}

/// A plain `bool` read into an `Option` (RON wants `Some(..)` for an `Option` otherwise).
fn some_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    bool::deserialize(d).map(Some)
}

impl Tiles {
    /// Keep scheme and colours consistent after an edit that started from `before`: a new
    /// scheme starts over with its own colour count, a colour added resamples the scheme,
    /// and removing or editing a colour makes it "Custom" (unless that's the scheme's own).
    pub fn reconcile(&mut self, before: &Tiles) {
        if self.scheme != before.scheme {
            if let Some(c) = self
                .scheme
                .native_count()
                .and_then(|n| self.scheme.colors(n))
            {
                self.colors = c;
            }
        } else if removed_one(&before.colors, &self.colors) {
            // A colour taken out of the list: keep the rest as they are.
            if self.scheme.colors(self.colors.len()).as_ref() != Some(&self.colors) {
                self.scheme = Scheme::Custom;
            }
        } else if self.colors.len() != before.colors.len() {
            if let Some(c) = self.scheme.colors(self.colors.len().max(1)) {
                self.colors = c;
            }
        } else if self.colors != before.colors {
            self.scheme = Scheme::Custom;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

    #[test]
    fn removing_a_colour_keeps_the_others() {
        let before = Tiles::default();
        let mut t = before.clone();
        t.colors.remove(0);
        t.reconcile(&before);
        assert_eq!(t.colors, before.colors[1..]);
        assert_eq!(t.scheme, Scheme::Custom);
        // The last one gone is still the scheme's own list.
        let mut t = before.clone();
        t.colors.pop();
        t.reconcile(&before);
        assert_eq!(t.scheme, Scheme::Classic);
    }

    #[test]
    fn tiles_reconcile() {
        let before = Tiles::default();
        let mut t = before.clone();
        t.scheme = Scheme::Spectral;
        t.reconcile(&before);
        assert_eq!(t.colors, Scheme::Spectral.colors(11).unwrap());

        // A new scheme resets the count, even after colours were added.
        let before = t.clone();
        t.scheme = Scheme::Turbo;
        t.reconcile(&before);
        assert_eq!(t.colors, Scheme::Turbo.colors(8).unwrap());

        let before = t.clone();
        t.colors.push(Color32::RED);
        t.reconcile(&before);
        assert_eq!((t.scheme, t.colors.len()), (Scheme::Turbo, 9));

        let before = t.clone();
        t.colors[0] = Color32::RED;
        t.reconcile(&before);
        assert_eq!(t.scheme, Scheme::Custom);
    }
}
