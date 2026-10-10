//! Animated camera moves.

use super::Fit;
use crate::core::geometry::{Point, Rect, Size, Vec2};
use crate::core::layout::{Reshape, Scene};
use crate::utils::math::{ease_out_cubic, geo_lerp};
use std::time::Instant;

/// Camera move to a folder: an even zoom about the move's fixed point (so the folder's centre
/// travels in a straight line and the root keeps covering the view), with the folder's box
/// changing shape on the way.
pub struct Anim {
    pub start: Instant,
    /// Length in seconds; 0 or less finishes at once.
    pub duration: f32,
    pub target: Vec<usize>,
    pub from_cam: Rect,
    /// Target folder's natural centre at start / end.
    pub n0: Point,
    pub n1: Point,
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
        self.progress() >= 1.0
    }

    /// 0..=1 through the move.
    fn progress(&self) -> f64 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.start.elapsed().as_secs_f32() / self.duration).min(1.0) as f64
    }

    /// Camera and reshapes partway through the move, in a `view`-sized window.
    pub fn state(&self, scene: &Scene, view: Size) -> (Rect, Vec<Reshape>) {
        let e = ease_out_cubic(self.progress());
        let (c0, c1) = (self.from_cam, self.end_cam);

        // Even zoom about the fixed point of the whole move (a plain pan if there's no zoom).
        let k = c1.width() / c0.width();
        let fp = |q: Point, e: f64| move_point(c0, c1, q, e);
        let mut cam = Rect::from_origin_size(fp(c0.origin(), e), c0.size() * k.powf(e));

        // Another folder's reshape fades out; that moves things inside it, so keep the target's
        // natural centre on its straight path regardless.
        let mut ovs: Vec<Reshape> = self.old.iter().map(|o| o.faded(1.0 - e)).collect();
        if !ovs.is_empty() {
            let want = fp(self.n0, e) + (self.n1 - fp(self.n0, 1.0)) * e;
            if let Some(n) = scene.locate(cam, &self.target, &ovs) {
                let o = cam.origin() + (want - n.center());
                let x = o.x.min(0.0).max(view.width - cam.width());
                let y = o.y.min(0.0).max(view.height - cam.height());
                cam = cam.with_origin((x, y));
            }
        }

        // The target's box changes shape evenly between its start and end reshape.
        if !self.target.is_empty() {
            let (a, b) = (&self.ov0, &self.ov1);
            ovs.push(Reshape {
                path: self.target.clone(),
                a: Vec2::new(geo_lerp(a.a.x, b.a.x, e), geo_lerp(a.a.y, b.a.y, e)),
                d: a.d.lerp(b.d, e),
            });
        }
        (cam, ovs)
    }
}

/// Where point `q` is at progress `e` of a camera move from `c0` to `c1` (same aspect):
/// an even zoom about the move's fixed point, or a plain pan when the scale doesn't change.
fn move_point(c0: Rect, c1: Rect, q: Point, e: f64) -> Point {
    let k = c1.width() / c0.width();
    if (k - 1.0).abs() < 1e-9 {
        return q + (c1.origin() - c0.origin()) * e;
    }
    // The fixed point p of the move satisfies c1 = p + k * (c0 - p).
    let p = ((c1.origin().to_vec2() - c0.origin().to_vec2() * k) / (1.0 - k)).to_point();
    p + (q - p) * k.powf(e)
}
