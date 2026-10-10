//! Holding the view steady while the window is resized.

use super::Camera;
use crate::constants::{ANCHOR_MIN_SHARE, RESIZE_SETTLE};
use crate::core::geometry::Rect;
use crate::core::layout::{Item, Scene};
use std::time::Instant;

/// What to hold steady while the window is resized.
#[derive(Clone)]
pub struct Anchor {
    pub path: Vec<usize>,
    /// Box area as a share of the window area.
    pub share: f64,
    /// Box centre as a fraction of the window size.
    pub u: f64,
    pub v: f64,
}

impl Camera {
    /// Deepest folder at the window centre that covers a good share of it.
    fn pick_anchor(&self, scene: &Scene, items: &[Item]) -> Option<Anchor> {
        let cam = self.cam?;
        let (vw, vh) = self.view;
        let (cx, cy) = (vw / 2.0, vh / 2.0);
        let ovs = self.ovs_at(cam);
        let it = items
            .iter()
            .filter(|it| it.is_folder && it.labeled && it.index.is_some() && it.contains(cx as f32, cy as f32))
            .filter(|it| {
                let w = (it.x + it.w).min(vw as f32) - it.x.max(0.0);
                let h = (it.y + it.h).min(vh as f32) - it.y.max(0.0);
                (w.max(0.0) * h.max(0.0)) as f64 >= ANCHOR_MIN_SHARE * vw * vh
            })
            .max_by_key(|it| it.folder.len())?;
        let path = it.path()?;
        let b = scene.locate(cam, &path, &ovs)?;
        let (bx, by) = b.center();
        Some(Anchor { path, share: b.area() / (vw * vh), u: bx / vw, v: by / vh })
    }

    /// The view changed size: refit, and hold the anchor folder's share and position.
    /// Call `finish_anim` first.
    pub fn resized(&mut self, scene: &Scene, items: &[Item], vw: f64, vh: f64) {
        let (ow, oh) = self.view;
        let Some(cam) = self.cam.filter(|_| ow > 0.0 && oh > 0.0) else {
            self.view = (vw, vh);
            self.cam = None;
            return;
        };
        let was_zoomed = self.zoomed();
        let anchor = match &self.resize_anchor {
            Some((a, at)) if at.elapsed() < RESIZE_SETTLE => a.clone(),
            _ => self.pick_anchor(scene, items),
        };
        self.resize_anchor = Some((anchor.clone(), Instant::now()));

        // Same zoom, same relative spot.
        let s = cam.w / ow;
        let (rx, ry) = ((ow / 2.0 - cam.x) / cam.w, (oh / 2.0 - cam.y) / cam.h);
        self.view = (vw, vh);
        let (w, h) = (vw * s, vh * s);
        let mut c = Rect::new(vw / 2.0 - rx * w, vh / 2.0 - ry * h, w, h);
        if !was_zoomed {
            self.cam = Some(self.full_view());
            return;
        }
        self.cam = Some(c);
        // A fitted folder is refitted to the new window shape.
        if let Some(f) = &self.fit {
            let path = f.reshape.path.clone();
            self.fit = self.solve_fit(scene, &path).map(|(_, f)| f);
        }
        if let Some(a) = anchor {
            for _ in 0..12 {
                let Some(b) = scene.locate(c, &a.path, &self.ovs_at(c)) else { break };
                if b.area() <= 0.0 {
                    break;
                }
                let k = (a.share * vw * vh / b.area()).sqrt();
                let bc = b.center();
                c = Self::scaled(c, bc, k);
                c.x += a.u * vw - bc.0;
                c.y += a.v * vh - bc.1;
            }
        }
        self.cam = Some(self.clamp_cam(scene, c));
        self.drop_faded_fit();
    }
}
