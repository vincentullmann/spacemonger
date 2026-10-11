//! Treemap-style drawing on pixel-snapped coordinates (port of
//! FolderView.minimalDrawDisplayFolder & friends).

mod boxes;
mod labels;

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Galley, Painter, Pos2, Stroke, Vec2};

use crate::helpers::egui::snap;
use crate::ui::palette::Palette;

/// Opacity of the drop shadow under light / dark label text.
const SHADOW_ALPHA_DARK: u8 = 170;
const SHADOW_ALPHA_LIGHT: u8 = 150;

/// Draws boxes and labels in view coordinates relative to `origin` (snapped to the pixel grid).
pub struct MapPainter<'a> {
    painter: &'a Painter,
    origin: Pos2,
    ppp: f32,
    pal: &'a Palette,
    font: FontId,
}

impl<'a> MapPainter<'a> {
    /// A painter whose view origin is `min` (the allocated rect's top-left).
    pub fn new(painter: &'a Painter, min: Pos2, ppp: f32, pal: &'a Palette, font: FontId) -> Self {
        Self {
            painter,
            origin: snap(min, ppp),
            ppp,
            pal,
            font,
        }
    }

    /// The colours this painter draws with.
    pub fn palette(&self) -> &Palette {
        self.pal
    }

    /// Screen position to view coordinates.
    pub fn local(&self, p: Pos2) -> (f32, f32) {
        (p.x - self.origin.x, p.y - self.origin.y)
    }

    fn snap(&self, p: Pos2) -> Pos2 {
        snap(p, self.ppp)
    }

    /// Screen rect for a view-coordinate box, snapped to the pixel grid.
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> egui::Rect {
        let r = egui::Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
            .translate(self.origin.to_vec2());
        egui::Rect::from_min_max(self.snap(r.min), self.snap(r.max))
    }

    /// Fill a view-coordinate box with `c` (nothing for an empty box).
    pub fn fill(&self, c: Color32, x: f32, y: f32, w: f32, h: f32) {
        if w > 0.0 && h > 0.0 {
            self.painter.rect_filled(self.rect(x, y, w, h), 0.0, c);
        }
    }

    /// `s` laid out in the label font, once: measure it with [`Self::size`], draw it with
    /// [`Self::text`] (in any colour, shadow included).
    pub fn galley(&self, s: &str) -> Arc<Galley> {
        self.painter
            .layout_no_wrap(s.to_owned(), self.font.clone(), Color32::WHITE)
    }

    /// Size of `s` in the label font, rounded up to whole points.
    pub fn text_width(&self, s: &str) -> (f32, f32) {
        Self::size(&self.galley(s))
    }

    /// A galley's size, rounded up to whole points.
    pub fn size(g: &Galley) -> (f32, f32) {
        (g.size().x.ceil(), g.size().y.ceil())
    }

    /// Draw a label galley with its top-left at view point (`x`, `y`), plus the drop shadow
    /// if it's on.
    fn text(&self, p: &Painter, g: &Arc<Galley>, x: f32, y: f32, c: Color32) {
        let pos = self.snap(self.origin + Vec2::new(x, y));
        if self.pal.shadow {
            // Dark shadow under light text, light under dark.
            let shadow = if c.intensity() > 0.5 {
                Color32::from_black_alpha(SHADOW_ALPHA_DARK)
            } else {
                Color32::from_white_alpha(SHADOW_ALPHA_LIGHT)
            };
            p.galley_with_override_text_color(pos + Vec2::splat(1.0), g.clone(), shadow);
        }
        p.galley_with_override_text_color(pos, g.clone(), c);
    }

    /// A path-bar box in the treemap's style: fill, border (the hover outline when hovered), label
    /// on the left.
    #[allow(clippy::too_many_arguments)]
    pub fn cell(&self, x: f32, y: f32, w: f32, h: f32, color: Color32, hover: bool, label: &str) {
        let fill = if hover {
            color.lerp_to_gamma(Color32::WHITE, self.pal.hover)
        } else {
            color
        };
        self.fill(fill, x + 1.0, y + 1.0, w - 1.0, h - 1.0);
        let (bc, bw) = if hover {
            (self.pal.hover_border, self.pal.hover_width)
        } else {
            (self.pal.border, self.pal.border_width)
        };
        self.painter.rect_stroke(
            self.rect(x + 1.0, y + 1.0, w - 1.0, h - 1.0),
            0.0,
            Stroke::new(bw / self.ppp, bc),
            egui::StrokeKind::Inside,
        );
        let label = self.galley(label);
        let (_, th) = Self::size(&label);
        let p = self
            .painter
            .with_clip_rect(self.rect(x, y, w, h).intersect(self.painter.clip_rect()));
        self.text(
            &p,
            &label,
            x + 6.0,
            y + 1.0 + (h - 1.0 - th) / 2.0,
            self.pal.bar_text_on(fill),
        );
    }

    /// Rectangle-selection overlay between two view points.
    pub fn marquee(&self, a: (f32, f32), b: (f32, f32)) {
        let r = self.rect(
            a.0.min(b.0),
            a.1.min(b.1),
            (a.0 - b.0).abs(),
            (a.1 - b.1).abs(),
        );
        let c = self.pal.text;
        self.painter.rect_filled(r, 0.0, c.gamma_multiply(0.12));
        self.painter
            .rect_stroke(r, 0.0, Stroke::new(1.0, c), egui::StrokeKind::Inside);
    }
}
