//! Animated camera moves.

use super::Fit;
use crate::constants::ANIM_DURATION;
use crate::core::geometry::Rect;
use crate::core::layout::{Reshape, Scene};
use crate::utils::math::{ease_out_cubic, geo_lerp, lerp};
use std::time::Instant;

/// Camera move to a folder: an even zoom about the move's fixed point (so the folder's centre
/// travels in a straight line and the root keeps covering the view), with the folder's box
/// changing shape on the way.
pub struct Anim {
    pub start: Instant,
    pub target: Vec<usize>,
    pub from_cam: Rect,
    /// Target folder's natural centre at start / end.
    pub n0: (f64, f64),
    pub n1: (f64, f64),
    /// Target's reshape relative to its natural box, at start / end.
    pub ov0: Reshape,
    pub ov1: Reshape,
    /// Another folder's reshape fading out during the move.
    pub old: Option<Reshape>,
    pub end_cam: Rect,
    pub end_fit: Option<Fit>,
    /// Keep the selection when the move ends (framing), rather than clearing it (zooming in).
    pub keep_sel: bool,
}

impl Anim {
    pub fn done(&self) -> bool {
        self.start.elapsed().as_secs_f32() >= ANIM_DURATION
    }

    /// Camera and reshapes partway through the move, in a `view`-sized window.
    pub fn state(&self, scene: &Scene, view: (f64, f64)) -> (Rect, Vec<Reshape>) {
        let (vw, vh) = view;
        let t = (self.start.elapsed().as_secs_f32() / ANIM_DURATION).min(1.0) as f64;
        let e = ease_out_cubic(t);
        let (c0, c1) = (self.from_cam, self.end_cam);

        // Even zoom about the fixed point of the whole move (a plain pan if there's no zoom).
        let k = c1.w / c0.w;
        let fp = |q: (f64, f64), e: f64| move_point(c0, c1, q, e);
        let (x, y) = fp((c0.x, c0.y), e);
        let r = k.powf(e);
        let mut cam = Rect::new(x, y, c0.w * r, c0.h * r);

        // Another folder's reshape fades out; that moves things inside it, so keep the target's
        // natural centre on its straight path regardless.
        let mut ovs: Vec<Reshape> = self.old.iter().map(|o| o.faded(1.0 - e)).collect();
        if !ovs.is_empty() {
            let (end, want) = (fp(self.n0, 1.0), fp(self.n0, e));
            let want = (want.0 + (self.n1.0 - end.0) * e, want.1 + (self.n1.1 - end.1) * e);
            if let Some(n) = scene.locate(cam, &self.target, &ovs) {
                let nc = n.center();
                cam.x = (cam.x + want.0 - nc.0).min(0.0).max(vw - cam.w);
                cam.y = (cam.y + want.1 - nc.1).min(0.0).max(vh - cam.h);
            }
        }

        // The target's box changes shape evenly between its start and end reshape.
        if !self.target.is_empty() {
            let (a, b) = (&self.ov0, &self.ov1);
            ovs.push(Reshape {
                path: self.target.clone(),
                a: (geo_lerp(a.a.0, b.a.0, e), geo_lerp(a.a.1, b.a.1, e)),
                d: (lerp(a.d.0, b.d.0, e), lerp(a.d.1, b.d.1, e)),
            });
        }
        (cam, ovs)
    }
}

/// Where point `q` is at progress `e` of a camera move from `c0` to `c1` (same aspect):
/// an even zoom about the move's fixed point, or a plain pan when the scale doesn't change.
fn move_point(c0: Rect, c1: Rect, q: (f64, f64), e: f64) -> (f64, f64) {
    let k = c1.w / c0.w;
    if (k - 1.0).abs() < 1e-9 {
        return (q.0 + (c1.x - c0.x) * e, q.1 + (c1.y - c0.y) * e);
    }
    let p = ((c1.x - k * c0.x) / (1.0 - k), (c1.y - k * c0.y) / (1.0 - k));
    let r = k.powf(e);
    (p.0 + r * (q.0 - p.0), p.1 + r * (q.1 - p.1))
}
