//! Box fills and outlines.

use super::MapPainter;
use crate::core::layout::Item;
use crate::core::model::Tree;
use eframe::egui::{self, Color32, Stroke};

/// Boxes this small (plus their 1-point gap) get no outline.
const MIN_OUTLINED: f32 = 4.0;

impl MapPainter<'_> {
    /// Borders on and no gap: neighbouring boxes share one border line, like a table with
    /// collapsed borders.
    pub fn collapsed(&self) -> bool {
        self.pal.borders && self.pal.gap <= 0.0
    }

    /// The filled part of an item's box: it gives up `gap` of its width and height to the
    /// space between neighbours, most of it on the top / left (all of it for the default 1).
    fn fill_box(&self, it: &Item) -> (f32, f32, f32, f32) {
        let g = self.pal.gap;
        let off = (g / 2.0).ceil();
        (it.x + off, it.y + off, it.w - g, it.h - g)
    }

    /// An item's border just inside its fill (the hover outline when hovered), its width in
    /// physical pixels. With collapsed borders the box reaches one border width further right
    /// and down, onto the neighbour's line.
    pub fn outline(&self, it: &Item, sel: bool, hover: bool) {
        let pal = self.pal;
        let (fx, fy, mut fw, mut fh) = self.fill_box(it);
        if it.w + 1.0 <= MIN_OUTLINED
            || it.h + 1.0 <= MIN_OUTLINED
            || fw <= 0.0
            || fh <= 0.0
            || !(pal.borders || sel || hover)
        {
            return;
        }
        if self.collapsed() {
            fw += pal.border_width / self.ppp;
            fh += pal.border_width / self.ppp;
        }
        let border_color = if sel {
            pal.selection
        } else if hover {
            pal.hover_border
        } else {
            pal.border
        };
        let border_width = if hover {
            pal.hover_width
        } else {
            pal.border_width
        };
        self.painter.rect_stroke(
            self.rect(fx, fy, fw, fh),
            0.0,
            Stroke::new(border_width / self.ppp, border_color),
            egui::StrokeKind::Inside,
        );
    }

    /// Flat box: a single fill with a gap to its neighbours (1px by default), plus its label.
    pub fn item(&self, tree: &Tree, it: &Item, sel: bool, hover: bool) {
        let pal = self.pal;

        if !it.is_free {
            let color = if sel {
                pal.selection
            } else if hover {
                pal.depth(it.depth).lerp_to_gamma(Color32::WHITE, pal.hover)
            } else {
                pal.depth(it.depth)
            };
            let (fx, fy, fw, fh) = self.fill_box(it);
            self.fill(color, fx, fy, fw, fh);
            // Collapsed borders: a later neighbour paints over this box's right / bottom line,
            // so hover and selection outlines are drawn afterwards (see `outline`).
            if !(self.collapsed() && (sel || hover)) {
                self.outline(it, sel, hover);
            }
        }
        if it.labeled {
            self.label(tree, it, sel);
        }
    }
}
