//! Camera moves that frame something: zoom to a folder, frame a set of entries, zoom out.

use super::{Anim, Camera, Fit};
use crate::constants::{FRAME_FILL, MAX_ZOOM};
use crate::core::geometry::Rect;
use crate::core::layout::{self, Reshape, Scene};
use std::time::Instant;

impl Camera {
    /// Even-zoom camera that centres the bounding box of the entries at `paths` (natural boxes)
    /// and fits it in the view with a margin, as far as the root allows.
    fn solve_frame(&self, scene: &Scene, paths: &[Vec<usize>]) -> Option<Rect> {
        let (vw, vh) = self.view;
        let bbox = |cam: Rect| {
            let mut bb: Option<(f64, f64, f64, f64)> = None;
            for r in paths.iter().filter_map(|p| scene.locate(cam, p, &[])) {
                let (x0, y0, x1, y1) = bb.unwrap_or((r.x, r.y, r.x + r.w, r.y + r.h));
                bb = Some((x0.min(r.x), y0.min(r.y), x1.max(r.x + r.w), y1.max(r.y + r.h)));
            }
            bb.map(|(x0, y0, x1, y1)| Rect::new(x0, y0, x1 - x0, y1 - y0)).filter(|b| b.w > 1e-9 && b.h > 1e-9)
        };
        // Folder frames don't scale with the camera, so converge on it.
        let mut cam = self.cam?;
        for _ in 0..40 {
            let b = bbox(cam)?;
            let k = (FRAME_FILL * vw / b.w).min(FRAME_FILL * vh / b.h).min(MAX_ZOOM * vw / cam.w);
            let bc = b.center();
            cam = Self::scaled(cam, bc, k);
            cam.x += vw / 2.0 - bc.0;
            cam.y += vh / 2.0 - bc.1;
            if (k - 1.0).abs() < 1e-9 && (vw / 2.0 - bc.0).abs() < 1e-6 && (vh / 2.0 - bc.1).abs() < 1e-6 {
                break;
            }
        }
        Some(self.cover_root(cam))
    }

    /// Even-zoom camera at which the folder's natural box has the window's area and sits in
    /// the middle (as far as the root allows), plus the reshape that makes it fill the window.
    pub(super) fn solve_fit(&self, scene: &Scene, path: &[usize]) -> Option<(Rect, Fit)> {
        let t = self.fill_box();
        let mut cam = self.cam?;
        for _ in 0..40 {
            let n = scene.locate(cam, path, &[])?;
            if n.area() <= 0.0 {
                return None;
            }
            let k = (t.area() / n.area()).sqrt();
            let (nc, tc) = (n.center(), t.center());
            cam = Self::scaled(cam, nc, k);
            cam.x += tc.0 - nc.0;
            cam.y += tc.1 - nc.1;
            if (k - 1.0).abs() < 1e-9 && (tc.0 - nc.0).abs() < 1e-6 && (tc.1 - nc.1).abs() < 1e-6 {
                break;
            }
        }
        // Root coverage only (the fit itself isn't active yet).
        let cam = self.cover_root(cam);
        let n = scene.locate(cam, path, &[])?;
        let reshape = Reshape::between(path.to_vec(), n, t);
        Some((cam, Fit { reshape, scale: self.scale_of(cam) }))
    }

    /// Animate to the folder at `path`, filling the view. Call `finish_anim` first.
    pub fn zoom_to(&mut self, scene: &Scene, path: &[usize]) {
        let Some(cam0) = self.cam else { return };
        let ovs0 = self.ovs_at(cam0);
        let Some(b0) = scene.locate(cam0, path, &ovs0) else { return };
        // Any other reshaped folder fades out on the way.
        let old: Option<Reshape> = ovs0.iter().find(|o| o.path != path).cloned();
        let Some(n0) = scene.locate(cam0, path, old.as_slice()) else { return };

        let (end_cam, end_fit) = if path.is_empty() {
            (self.full_view(), None)
        } else {
            let Some((c, f)) = self.solve_fit(scene, path) else { return };
            (c, Some(f))
        };
        let Some(n1) = scene.locate(end_cam, path, &[]) else { return };
        let b1 = if path.is_empty() { end_cam } else { self.fill_box() };
        if end_cam == cam0 && old.is_none() && (b0.x - b1.x).abs() < 0.5 && (b0.w - b1.w).abs() < 0.5 && (b0.h - b1.h).abs() < 0.5 {
            self.fit = end_fit;
            return;
        }
        self.anim = Some(Anim {
            start: Instant::now(),
            target: path.to_vec(),
            from_cam: cam0,
            n0: n0.center(),
            n1: n1.center(),
            ov0: Reshape::between(path.to_vec(), n0, b0),
            ov1: Reshape::between(path.to_vec(), n1, b1),
            old,
            end_cam,
            end_fit,
            keep_sel: false,
        });
    }

    /// Animate to frame the bounding box of the entries at `paths`. Call `finish_anim` first.
    pub fn frame(&mut self, scene: &Scene, paths: &[Vec<usize>]) {
        let (Some(cam0), Some(end_cam)) = (self.cam, self.solve_frame(scene, paths)) else { return };
        let old = self.ovs_at(cam0).into_iter().next();
        if end_cam == cam0 && old.is_none() {
            self.fit = None;
            return;
        }
        self.anim = Some(Anim {
            start: Instant::now(),
            target: Vec::new(),
            from_cam: cam0,
            n0: cam0.center(),
            n1: end_cam.center(),
            ov0: Reshape::between(Vec::new(), cam0, cam0),
            ov1: Reshape::between(Vec::new(), end_cam, end_cam),
            old,
            end_cam,
            end_fit: None,
            keep_sel: true,
        });
    }

    /// Make the move in progress (if any) keep the selection when it ends.
    pub fn keep_selection(&mut self) {
        if let Some(a) = &mut self.anim {
            a.keep_sel = true;
        }
    }

    /// Where Zoom Out goes from the folder at `zoom`: that folder if it isn't fitted to the
    /// view yet, otherwise its parent. `None` without a camera or tree.
    pub fn zoom_out_target(&self, scene: &Scene, zoom: &[usize]) -> Option<Vec<usize>> {
        let cam = self.cam?;
        scene.root?;
        let view = layout::root_content(self.full_view());
        let ovs = self.ovs_at(cam);
        let fitted = scene.content_of(cam, zoom, &ovs).is_some_and(|c| {
            (c.x - view.x).abs() < 0.5 && (c.y - view.y).abs() < 0.5 && (c.w - view.w).abs() < 0.5 && (c.h - view.h).abs() < 0.5
        });
        let mut target = zoom.to_vec();
        if fitted {
            target.pop();
        }
        Some(target)
    }
}
