//! Box labels: entry names with the chosen details under them, and the free-space block's
//! summary.

use super::MapPainter;
use crate::core::layout::Item;
use crate::core::model::{Entry, Tree};
use crate::utils::format;
use eframe::egui::Painter;

/// Narrowest file box that gets details under its name.
const MIN_DETAILS_W: f32 = 48.0;
/// Height one label line needs, at the original 10 pt (scaled with the font).
const LINE_H: f32 = 12.0;
/// Inset of a left-aligned label, and of a folder's title.
const INSET_X: f32 = 3.0;
const TITLE_Y: f32 = 2.0;
/// Offsets of the free-space block's four lines from its centre, at 10 pt.
const FREE_LINES_DY: [f32; 4] = [-18.0, -6.0, 6.0, 15.0];

/// Where text of length `len` starts in a span from `start` that's `avail` long: centred,
/// or `inset` in from the start when it doesn't fit (with a 1-point margin each side).
fn centre_or_start(start: f32, avail: f32, len: f32, inset: f32) -> f32 {
    if len > avail - 2.0 {
        start + inset
    } else {
        start + (avail - len) / 2.0
    }
}

impl MapPainter<'_> {
    /// The label of a laid-out box, clipped to it.
    pub(super) fn label(&self, tree: &Tree, it: &Item, sel: bool) {
        let Some(entry) = it.index.and_then(|i| tree.entry_at(&it.folder, i)) else {
            return;
        };
        let (x, y, w, h) = (it.x, it.y, it.w + 1.0, it.h + 1.0);
        let p = self
            .painter
            .with_clip_rect(self.rect(x, y, w, h).intersect(self.painter.clip_rect()));
        if it.is_free {
            self.free_label(&p, tree, (x, y, w, h));
        } else {
            self.entry_label(&p, tree, it, entry, sel);
        }
    }

    /// The free-space block: share of the drive, free bytes, file and folder counts.
    fn free_label(&self, p: &Painter, tree: &Tree, (x, y, w, h): (f32, f32, f32, f32)) {
        let lines = [
            format!(
                "<Free Space: {}>",
                format::size_string(tree.free_space, tree.total_space.max(1), true)
            ),
            format!(
                "{} Free",
                format::size_string(tree.free_space, tree.total_space, false)
            ),
            format!("Files Total:  {}", tree.num_files),
            format!("Folders Total:  {}", tree.num_folders),
        ]
        .map(|l| self.galley(&l));
        let (lw, lh) = Self::size(&lines[0]);
        let tx = centre_or_start(x, w, lw, 2.0);
        let ty = centre_or_start(y, h, lh, 1.0);
        for (line, dy) in lines.iter().zip(FREE_LINES_DY) {
            self.text(p, line, tx, ty + dy * self.pal.text_scale, self.pal.text);
        }
    }

    /// An entry's name (a folder's in its title bar, a file's centred), and under a file's
    /// name the details picked in the settings, as many as fit.
    fn entry_label(&self, p: &Painter, tree: &Tree, it: &Item, entry: &Entry, sel: bool) {
        let pal = self.pal;
        let (x, y, w, h) = (it.x, it.y, it.w + 1.0, it.h + 1.0);
        let fg = if sel { pal.selection_text } else { pal.text };
        let k = pal.text_scale;

        let name = self.galley(&entry.display_name());
        let (tw, th) = Self::size(&name);
        let (tx, mut ty) = if it.is_folder {
            (x + INSET_X, y + TITLE_Y)
        } else {
            (
                centre_or_start(x, w, tw, INSET_X),
                centre_or_start(y, h, th, TITLE_Y),
            )
        };

        let show_name = it.is_folder || pal.shown.name;
        let mut extras = if it.is_folder || w < MIN_DETAILS_W {
            Vec::new()
        } else {
            self.details(tree, it, entry)
        };
        let lines = |extras: &Vec<String>| extras.len() + show_name as usize;
        while lines(&extras) > 1 && h < LINE_H * k * lines(&extras) as f32 {
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
            let s = self.galley(s);
            let (sw, _) = Self::size(&s);
            let sx = centre_or_start(x, w, sw, INSET_X);
            self.text(p, &s, sx, ty + first + step * i as f32, fg);
        }
        ty += name_dy;
        if show_name {
            self.text(p, &name, tx, ty, fg);
        }
    }

    /// The details the settings show under a file's name, in order.
    fn details(&self, tree: &Tree, it: &Item, entry: &Entry) -> Vec<String> {
        let shown = &self.pal.shown;
        let mut out = Vec::new();
        if shown.path {
            out.push(tree.full_path(&it.folder, it.index).display().to_string());
        }
        if shown.size {
            out.push(format::file_size(entry.actual));
        }
        if shown.modified {
            out.push(format::date(entry.mtime));
        }
        if shown.created && entry.created != 0 {
            out.push(format::date(entry.created));
        }
        out
    }
}
