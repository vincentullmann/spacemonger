//! Camera state and the direct manipulations: pan, wheel zoom, reveal.

use super::{Anchor, Anim, Fit};
use crate::constants::MAX_ZOOM;
use crate::core::geometry::Rect;
use crate::core::layout::{self, child_boxes, Item, Reshape, Scene};
use crate::core::model::Kind;
use std::time::Instant;

#[derive(Default)]
pub struct Camera {
    /// View size (points) the camera was last fitted to.
    pub view: (f64, f64),
    /// Box of the scan root in view coordinates; always the view's shape, scaled evenly.
    /// Equal to the view when fully zoomed out. `None` until the first frame.
    pub cam: Option<Rect>,
    /// Folder currently stretched to fill the window, if any.
    pub fit: Option<Fit>,
    /// Move in progress.
    pub anim: Option<Anim>,
    /// Anchor held during a window resize, and when the last resize step happened.
    pub(super) resize_anchor: Option<(Option<Anchor>, Instant)>,
}

impl Camera {
    /// Forget the camera position (new scan). Keeps the view size.
    pub fn reset(&mut self) {
        self.cam = None;
        self.fit = None;
        self.anim = None;
    }

    /// Start fully zoomed out if there's no camera yet.
    pub fn ensure(&mut self) {
        if self.cam.is_none() {
            self.cam = Some(self.full_view());
        }
    }

    pub fn full_view(&self) -> Rect {
        Rect::new(0.0, 0.0, self.view.0, self.view.1)
    }

    /// Box a folder needs so its content exactly fills the view.
    pub fn fill_box(&self) -> Rect {
        Rect::new(-3.0, -12.0, self.view.0 + 5.0, self.view.1 + 14.0)
    }

    pub fn scale_of(&self, c: Rect) -> f64 {
        c.w / self.view.0.max(1.0)
    }

    pub fn zoomed(&self) -> bool {
        self.fit.is_some() || self.cam.is_some_and(|c| c != self.full_view())
    }

    /// Reshapes in effect for camera `c` (outside animations).
    pub fn ovs_at(&self, c: Rect) -> Vec<Reshape> {
        match &self.fit {
            Some(f) => {
                let b = f.blend(self.scale_of(c));
                if b > 0.0 { vec![f.reshape.faded(b)] } else { Vec::new() }
            }
            None => Vec::new(),
        }
    }

    /// Camera and reshapes to draw this frame.
    pub fn view_state(&self, scene: &Scene) -> (Rect, Vec<Reshape>) {
        let cam = self.cam.unwrap_or(self.full_view());
        match &self.anim {
            Some(a) => a.state(scene, self.view),
            None => (cam, self.ovs_at(cam)),
        }
    }

    /// End the current move, jumping to where it was going. Returns true when the move asks
    /// for the selection to be cleared (zooming in), false otherwise.
    #[must_use]
    pub fn finish_anim(&mut self) -> bool {
        match self.anim.take() {
            Some(a) => {
                self.cam = Some(a.end_cam);
                self.fit = a.end_fit;
                !a.keep_sel
            }
            None => false,
        }
    }

    /// Scale camera `c` by `k` about point `p`.
    pub(super) fn scaled(c: Rect, p: (f64, f64), k: f64) -> Rect {
        Rect::new(p.0 + (c.x - p.0) * k, p.1 + (c.y - p.1) * k, c.w * k, c.h * k)
    }

    /// Keep the root covering the view (no zooming out past the scan root), and while a
    /// folder is fully fitted, keep it covering the view too.
    pub(super) fn clamp_cam(&self, scene: &Scene, mut c: Rect) -> Rect {
        let (vw, vh) = self.view;
        if c.w < vw || c.h < vh {
            return self.full_view();
        }
        // Allowed shifts: keep the root covering the view...
        let (mut xlo, mut xhi) = (vw - c.w - c.x, -c.x);
        let (mut ylo, mut yhi) = (vh - c.h - c.y, -c.y);
        // ...and a fully fitted folder's content too.
        if let Some(f) = self.fit.as_ref().filter(|f| f.blend(self.scale_of(c)) >= 1.0) {
            if let Some(b) = scene.locate(c, &f.reshape.path, std::slice::from_ref(&f.reshape)) {
                let k = layout::content(b);
                let view = layout::root_content(self.full_view());
                let (fxlo, fxhi) = (view.x + view.w - (k.x + k.w), view.x - k.x);
                let (fylo, fyhi) = (view.y + view.h - (k.y + k.h), view.y - k.y);
                if fxlo.max(xlo) <= fxhi.min(xhi) {
                    (xlo, xhi) = (fxlo.max(xlo), fxhi.min(xhi));
                }
                if fylo.max(ylo) <= fyhi.min(yhi) {
                    (ylo, yhi) = (fylo.max(ylo), fyhi.min(yhi));
                }
            }
        }
        c.x += 0f64.clamp(xlo, xhi.max(xlo));
        c.y += 0f64.clamp(ylo, yhi.max(ylo));
        c
    }

    /// Keep the root covering the view (ignoring any fitted folder).
    pub(super) fn cover_root(&self, mut c: Rect) -> Rect {
        let (vw, vh) = self.view;
        if c.w < vw || c.h < vh {
            return self.full_view();
        }
        c.x = c.x.min(0.0).max(vw - c.w);
        c.y = c.y.min(0.0).max(vh - c.h);
        c
    }

    /// Scale the camera by `k` about a view point, keeping whatever is under it in place.
    pub fn zoom_at(&mut self, scene: &Scene, items: &[Item], px: f64, py: f64, k: f64) {
        let Some(cam) = self.cam else { return };
        let (vw, vh) = self.view;
        let k = k.min(MAX_ZOOM * vw / cam.w).min(MAX_ZOOM * vh / cam.h);

        // Deepest entry under the point (drawn last).
        let anchor = items
            .iter()
            .rev()
            .find(|it| it.index.is_some() && !it.is_free && it.contains(px as f32, py as f32))
            .and_then(Item::path);
        let before = anchor.and_then(|path| scene.locate(cam, &path, &self.ovs_at(cam)).map(|b| (path, b)));

        let mut c = Self::scaled(cam, (px, py), k);
        // Frames don't scale (and a fitted folder's stretch fades), so pin the anchor.
        if let Some((path, b0)) = before.filter(|(_, b)| b.w > 1e-6 && b.h > 1e-6) {
            if let Some(b1) = scene.locate(c, &path, &self.ovs_at(c)) {
                let (u, v) = ((px - b0.x) / b0.w, (py - b0.y) / b0.h);
                c.x += px - (b1.x + u * b1.w);
                c.y += py - (b1.y + v * b1.h);
            }
        }
        self.cam = Some(self.clamp_cam(scene, c));
        self.drop_faded_fit();
    }

    /// Forget the fit once zoomed out far enough that it has faded away completely.
    pub(super) fn drop_faded_fit(&mut self) {
        if let (Some(f), Some(c)) = (&self.fit, self.cam) {
            if f.blend(self.scale_of(c)) <= 0.0 {
                self.fit = None;
            }
        }
    }

    pub fn pan(&mut self, scene: &Scene, dx: f64, dy: f64) {
        if let Some(c) = self.cam {
            self.cam = Some(self.clamp_cam(scene, Rect::new(c.x + dx, c.y + dy, c.w, c.h)));
        }
    }

    /// Pan the least amount that brings the entry at `path` fully into view (or, if it's
    /// bigger than the view, lines its top / left edge up with the view's).
    pub fn reveal(&mut self, scene: &Scene, path: &[usize]) {
        let Some(cam) = self.cam else { return };
        let Some(b) = scene.locate(cam, path, &self.ovs_at(cam)) else { return };
        let shift = |lo: f64, len: f64, view: f64| -> f64 {
            let hi = lo + len;
            if lo <= 0.0 && hi >= view && len > view {
                0.0 // already covers the view
            } else if len >= view {
                -lo
            } else {
                let m = 8f64.min((view - len) / 2.0);
                if lo < m {
                    m - lo
                } else if hi > view - m {
                    view - m - hi
                } else {
                    0.0
                }
            }
        };
        let (vw, vh) = self.view;
        let (dx, dy) = (shift(b.x, b.w, vw), shift(b.y, b.h, vh));
        if dx != 0.0 || dy != 0.0 {
            self.pan(scene, dx, dy);
        }
    }

    /// Boxes of the folder's entries as drawn now (natural split of its content area at the
    /// current camera), without free space.
    pub fn child_boxes(&self, scene: &Scene, folder: &[usize]) -> Vec<(usize, Rect)> {
        let (Some(root), Some(cam)) = (scene.root, self.cam) else { return Vec::new() };
        let Some(f) = root.descendant(folder) else { return Vec::new() };
        let Some(area) = scene.content_of(cam, folder, &self.ovs_at(cam)) else { return Vec::new() };
        child_boxes(f, area, scene.params)
            .into_iter()
            .filter(|&(i, _)| !matches!(f.entries[i].kind, Kind::Free))
            .collect()
    }
}
