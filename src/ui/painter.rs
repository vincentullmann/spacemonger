//! Treemap-style drawing on pixel-snapped coordinates (port of
//! FolderView.minimalDrawDisplayFolder & friends).

use crate::core::layout::Item;
use crate::core::model::Tree;
use crate::helpers::egui::snap;
use crate::ui::palette::Palette;
use crate::utils::format;
use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Stroke, Vec2};

/// Draws boxes and labels in view coordinates relative to `origin` (snapped to the pixel grid).
pub struct MapPainter<'a> {
    painter: &'a Painter,
    origin: Pos2,
    ppp: f32,
    pal: Palette,
    font: FontId,
}

impl<'a> MapPainter<'a> {
    /// A painter whose view origin is `min` (the allocated rect's top-left).
    pub fn new(painter: &'a Painter, min: Pos2, ppp: f32, pal: Palette, font: FontId) -> Self {
        Self {
            painter,
            origin: snap(min, ppp),
            ppp,
            pal,
            font,
        }
    }

    pub fn palette(&self) -> &Palette {
        &self.pal
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

    pub fn fill(&self, c: Color32, x: f32, y: f32, w: f32, h: f32) {
        if w > 0.0 && h > 0.0 {
            self.painter.rect_filled(self.rect(x, y, w, h), 0.0, c);
        }
    }

    pub fn text_width(&self, s: &str) -> (f32, f32) {
        let g = self
            .painter
            .layout_no_wrap(s.to_string(), self.font.clone(), Color32::WHITE);
        (g.size().x.ceil(), g.size().y.ceil())
    }

    fn text(&self, p: &Painter, s: &str, x: f32, y: f32, c: Color32) {
        let pos = self.snap(self.origin + Vec2::new(x, y));
        if self.pal.shadow {
            // Dark shadow under light text, light under dark.
            let shadow = if c.intensity() > 0.5 {
                Color32::from_black_alpha(170)
            } else {
                Color32::from_white_alpha(150)
            };
            p.text(
                pos + Vec2::splat(1.0),
                Align2::LEFT_TOP,
                s,
                self.font.clone(),
                shadow,
            );
        }
        p.text(pos, Align2::LEFT_TOP, s, self.font.clone(), c);
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
        let (_, th) = self.text_width(label);
        let p = self
            .painter
            .with_clip_rect(self.rect(x, y, w, h).intersect(self.painter.clip_rect()));
        self.text(
            &p,
            label,
            x + 6.0,
            y + 1.0 + (h - 1.0 - th) / 2.0,
            self.pal.text_on(fill),
        );
    }

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
        let pal = &self.pal;
        let (fx, fy, mut fw, mut fh) = self.fill_box(it);
        if it.w + 1.0 <= 4.0
            || it.h + 1.0 <= 4.0
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

    /// Flat box: a single fill with a gap to its neighbours (1px by default), plus its label.
    pub fn item(&self, tree: &Tree, it: &Item, sel: bool, hover: bool) {
        let pal = &self.pal;
        let (x, y, w, h) = (it.x, it.y, it.w + 1.0, it.h + 1.0);

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

        if !it.labeled {
            return;
        }
        let Some(entry) = it.index.and_then(|i| tree.entry_at(&it.folder, i)) else {
            return;
        };
        let p = self
            .painter
            .with_clip_rect(self.rect(x, y, w, h).intersect(self.painter.clip_rect()));
        let fg = if sel {
            pal.selection_text
        } else {
            pal.text_on(pal.depth(it.depth))
        };

        if it.is_free {
            let ts = tree.total_space.max(1);
            let fp = tree.free_space as u128 * 1000 / ts as u128;
            let lines = [
                format!("<Free Space: {}.{}%>", fp / 10, fp % 10),
                format!(
                    "{} Free",
                    format::size_string(tree.free_space, tree.total_space, false)
                ),
                format!("Files Total:  {}", tree.num_files),
                format!("Folders Total:  {}", tree.num_folders),
            ];
            let (lw, lh) = self.text_width(&lines[0]);
            let tx = if lw > w - 2.0 {
                x + 2.0
            } else {
                x + (w - lw) / 2.0
            };
            let ty = if lh > h - 2.0 {
                y + 1.0
            } else {
                y + (h - lh) / 2.0
            };
            for (line, dy) in lines.iter().zip([-18.0, -6.0, 6.0, 15.0]) {
                self.text(&p, line, tx, ty + dy * pal.text_scale, pal.text);
            }
            return;
        }

        let (tw, th) = self.text_width(&entry.name);
        let tx = if tw > w - 2.0 || it.is_folder {
            x + 3.0
        } else {
            x + (w - tw) / 2.0
        };
        let mut ty = if th > h - 2.0 || it.is_folder {
            y + 2.0
        } else {
            y + (h - th) / 2.0
        };

        // The details chosen for file labels, under the name, as far as the box is tall
        // enough. Folders only show their name in the title bar.
        let k = pal.text_scale;
        let shown = &pal.shown;
        let show_name = it.is_folder || shown.name;
        let mut extras: Vec<String> = Vec::new();
        if !it.is_folder && w >= 48.0 {
            if shown.path {
                extras.push(tree.full_path(&it.folder, it.index).display().to_string());
            }
            if shown.size {
                extras.push(format::file_size(entry.actual));
            }
            if shown.modified {
                extras.push(format::date(entry.mtime));
            }
            if shown.created && entry.created != 0 {
                extras.push(format::date(entry.created));
            }
        }
        let lines = |extras: &Vec<String>| extras.len() + show_name as usize;
        while lines(&extras) > 1 && h < 12.0 * k * lines(&extras) as f32 {
            extras.pop();
        }
        // Offsets from the centred line: the name a little apart from the details below it.
        let n = extras.len() as f32;
        let (name_dy, first, step) = match (show_name, extras.len()) {
            (true, 0) => (0.0, 0.0, 0.0),
            (true, 1) => (-6.0 * k, 6.0 * k, 0.0),
            (true, 2) => (-12.0 * k, 1.0, 10.0 * k),
            (true, _) => {
                let name_dy = -(6.0 * k + 5.0 * k * (n - 1.0));
                (name_dy, name_dy + 13.0 * k, 10.0 * k)
            }
            (false, _) => (0.0, -5.0 * k * (n - 1.0), 10.0 * k),
        };
        for (i, s) in extras.iter().enumerate() {
            let (sw, _) = self.text_width(s);
            let sx = if sw > w - 2.0 {
                x + 3.0
            } else {
                x + (w - sw) / 2.0
            };
            self.text(&p, s, sx, ty + first + step * i as f32, fg);
        }
        ty += name_dy;
        if !show_name {
            return;
        }
        self.text(&p, &entry.name, tx, ty, fg);
    }
}
