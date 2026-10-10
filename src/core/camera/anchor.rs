//! Holding the view steady while the window is resized.

use super::Camera;
use crate::constants::{ANCHOR_MIN_SHARE, RESIZE_SETTLE};
use crate::core::geometry::{Point, Rect, RectExt, Size};
use crate::core::layout::{Item, Scene};
use std::time::Instant;

/// What to hold steady while the window is resized.
#[derive(Clone)]
pub struct Anchor {
    pub path: Vec<usize>,
    /// Box area as a share of the window area.
    pub share: f64,
    /// Box centre as a fraction of the window size.
    pub at: Point,
}

impl Camera {
    /// Deepest folder at the window centre that covers a good share of it.
    fn pick_anchor(&self, scene: &Scene, items: &[Item]) -> Option<Anchor> {
        let cam = self.cam?;
        let Size {
            width: vw,
            height: vh,
        } = self.view;
        let (cx, cy) = (vw / 2.0, vh / 2.0);
        let ovs = self.ovs_at(cam);
        let it = items
            .iter()
            .filter(|it| {
                it.is_folder
                    && it.labeled
                    && it.index.is_some()
                    && it.contains(cx as f32, cy as f32)
            })
            .filter(|it| {
                let w = (it.x + it.w).min(vw as f32) - it.x.max(0.0);
                let h = (it.y + it.h).min(vh as f32) - it.y.max(0.0);
                (w.max(0.0) * h.max(0.0)) as f64 >= ANCHOR_MIN_SHARE * vw * vh
            })
            .max_by_key(|it| it.folder.len())?;
        let path = it.path()?;
        let b = scene.locate(cam, &path, &ovs)?;
        let bc = b.center();
        Some(Anchor {
            path,
            share: b.clamped_area() / (vw * vh),
            at: Point::new(bc.x / vw, bc.y / vh),
        })
    }

    /// The view changed size: refit, and hold the anchor folder's share and position.
    /// Call `finish_anim` first.
    pub fn resized(&mut self, scene: &Scene, items: &[Item], view: Size) {
        let (
            Size {
                width: ow,
                height: oh,
            },
            Size {
                width: vw,
                height: vh,
            },
        ) = (self.view, view);
        let Some(cam) = self.cam.filter(|_| ow > 0.0 && oh > 0.0) else {
            self.view = view;
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
        let s = cam.width() / ow;
        let (rx, ry) = (
            (ow / 2.0 - cam.x0) / cam.width(),
            (oh / 2.0 - cam.y0) / cam.height(),
        );
        self.view = view;
        let (w, h) = (vw * s, vh * s);
        let mut c = Rect::from_origin_size((vw / 2.0 - rx * w, vh / 2.0 - ry * h), (w, h));
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
                let Some(b) = scene.locate(c, &a.path, &self.ovs_at(c)) else {
                    break;
                };
                if b.clamped_area() <= 0.0 {
                    break;
                }
                let k = (a.share * vw * vh / b.clamped_area()).sqrt();
                let bc = b.center();
                c = Self::scaled(c, bc, k) + (Point::new(a.at.x * vw, a.at.y * vh) - bc);
            }
        }
        self.cam = Some(self.clamp_cam(scene, c));
        self.drop_faded_fit();
    }
}
