//! Camera moves that frame something: zoom to a folder, frame a set of entries, zoom out.

use super::{Anim, Camera, Fit};
use crate::constants::MAX_ZOOM;
use crate::core::geometry::{Rect, RectExt};
use crate::core::layout::{self, Reshape, Scene};
use std::time::Instant;

impl Camera {
    /// Even-zoom camera that centres the bounding box of the entries at `paths` (natural boxes)
    /// and fits it in the view with a margin, as far as the root allows.
    fn solve_frame(&self, scene: &Scene, paths: &[Vec<usize>]) -> Option<Rect> {
        let (vw, vh) = (self.view.width, self.view.height);
        let bbox = |cam: Rect| {
            paths
                .iter()
                .filter_map(|p| scene.locate(cam, p, &[]))
                .reduce(|a, b| a.union(b))
                .filter(|b| b.width() > 1e-9 && b.height() > 1e-9)
        };
        // Folder frames don't scale with the camera, so converge on it.
        let fill = self.params.frame_fill;
        let mut cam = self.cam?;
        for _ in 0..40 {
            let b = bbox(cam)?;
            let k = (fill * vw / b.width()).min(fill * vh / b.height()).min(MAX_ZOOM * vw / cam.width());
            let bc = b.center();
            let off = self.view.to_rect().center() - bc;
            cam = Self::scaled(cam, bc, k) + off;
            if (k - 1.0).abs() < 1e-9 && off.x.abs() < 1e-6 && off.y.abs() < 1e-6 {
                break;
            }
        }
        Some(self.cover_root(cam))
    }

    /// Even-zoom camera at which the folder's natural box has the window's area and sits in
    /// the middle (as far as the root allows), plus the reshape that makes it fill the window.
    pub(super) fn solve_fit(&self, scene: &Scene, path: &[usize]) -> Option<(Rect, Fit)> {
        let t = self.fill_box(scene.params.title_h);
        let mut cam = self.cam?;
        for _ in 0..40 {
            let n = scene.locate(cam, path, &[])?;
            if n.clamped_area() <= 0.0 {
                return None;
            }
            let k = (t.clamped_area() / n.clamped_area()).sqrt();
            let (nc, off) = (n.center(), t.center() - n.center());
            cam = Self::scaled(cam, nc, k) + off;
            if (k - 1.0).abs() < 1e-9 && off.x.abs() < 1e-6 && off.y.abs() < 1e-6 {
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
        let b1 = if path.is_empty() { end_cam } else { self.fill_box(scene.params.title_h) };
        let same = |a: f64, b: f64| (a - b).abs() < 0.5;
        if end_cam == cam0 && old.is_none() && same(b0.x0, b1.x0) && same(b0.width(), b1.width()) && same(b0.height(), b1.height()) {
            self.fit = end_fit;
            return;
        }
        self.anim = Some(Anim {
            start: Instant::now(),
            duration: self.params.anim_duration,
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
            duration: self.params.anim_duration,
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
        let same = |a: f64, b: f64| (a - b).abs() < 0.5;
        let fitted = scene.content_of(cam, zoom, &ovs).is_some_and(|c| {
            same(c.x0, view.x0) && same(c.y0, view.y0) && same(c.width(), view.width()) && same(c.height(), view.height())
        });
        let mut target = zoom.to_vec();
        if fitted {
            target.pop();
        }
        Some(target)
    }
}
