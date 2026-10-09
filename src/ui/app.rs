//! Main window: toolbar, treemap view, dialogs (port of AppController + FolderView).

use crate::constants::*;
use crate::ui::palette::Palette;
use crate::utils::format;
use crate::utils::math::{ease_out_cubic, geo_lerp, lerp};
use crate::utils::text::elide_start;
use crate::core::geometry::Rect;
use crate::core::layout::{self, Item, LayoutParams, Reshape};
use crate::core::fs::{self as scan, Drive, ScanControl};
use crate::core::actions::Action;
use crate::core::model::{self, EntryRef, Tree};
use crate::core::selection::{Marquee, Nav, Selection};
use eframe::egui::{
    self, Align2, Color32, FontId, Id, Order, Painter, Pos2, Sense, Stroke, Vec2,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

struct ScanJob {
    drive: Drive,
    ctl: Arc<ScanControl>,
    rx: mpsc::Receiver<Option<Tree>>,
}

struct DriveDialog {
    drives: Vec<Drive>,
    selected: Option<usize>,
    path: String,
}

/// A folder reshaped to fill the window. The camera itself always zooms evenly; only this
/// folder's box is stretched, by an amount that fades out as you zoom out from it.
#[derive(Clone)]
struct Fit {
    /// Full-strength reshape, relative to the folder's natural box.
    ov: Reshape,
    /// Camera scale (root width / view width) at which the folder exactly fills the view.
    scale: f64,
}

impl Fit {
    /// Strength of the reshape at camera scale `s`: 1 at or above the fit scale, fading to 0
    /// by half of it (but always 0 when fully zoomed out).
    fn blend(&self, s: f64) -> f64 {
        let lo = (self.scale / 2.0).max(1.0);
        if self.scale - lo < 1e-9 {
            return if s >= self.scale { 1.0 } else { 0.0 };
        }
        ((s / lo).ln() / (self.scale / lo).ln()).clamp(0.0, 1.0)
    }
}

/// Camera move to a folder: an even zoom about the move's fixed point (so the folder's centre
/// travels in a straight line and the root keeps covering the view), with the folder's box
/// changing shape on the way.
struct Anim {
    start: Instant,
    target: Vec<usize>,
    from_cam: Rect,
    /// Target folder's natural centre at start / end.
    n0: (f64, f64),
    n1: (f64, f64),
    /// Target's reshape relative to its natural box, at start / end.
    ov0: Reshape,
    ov1: Reshape,
    /// Another folder's reshape fading out during the move.
    old: Option<Reshape>,
    end_cam: Rect,
    end_fit: Option<Fit>,
    /// Keep the selection when the move ends (framing), rather than clearing it (zooming in).
    keep_sel: bool,
}

/// What to hold steady while the window is resized.
#[derive(Clone)]
struct Anchor {
    path: Vec<usize>,
    /// Box area as a share of the window area.
    share: f64,
    /// Box centre as a fraction of the window size.
    u: f64,
    v: f64,
}

pub struct SpaceMonger {
    tree: Option<Tree>,
    drive: Option<Drive>,
    /// Index path of the deepest folder covering the whole view (derived from `cam`).
    zoom: Vec<usize>,
    /// Box of the scan root in view coordinates; always the view's shape, scaled evenly.
    /// Equal to the view when fully zoomed out.
    cam: Option<Rect>,
    /// Folder currently stretched to fill the window, if any.
    fit: Option<Fit>,
    /// View size (points) the camera was last fitted to.
    view: (f64, f64),
    /// Anchor held during a window resize, and when the last resize step happened.
    resize_anchor: Option<(Option<Anchor>, Instant)>,

    items: Vec<Item>,
    layout_key: Option<(f32, f32, u64, Rect, Vec<Reshape>)>,
    generation: u64,

    selection: Selection,
    marquee: Option<Marquee>,
    hovered: Option<usize>,
    hover_since: Instant,
    anim: Option<Anim>,

    show_free: bool,
    dark: bool,
    applied_dark: Option<bool>,
    params: LayoutParams,

    dialog: Option<DriveDialog>,
    scan: Option<ScanJob>,
    confirm_delete: Option<Vec<(EntryRef, PathBuf)>>,
    error: Option<String>,
    title: String,
    font: FontId,
}

impl SpaceMonger {
    pub fn new(cc: &eframe::CreationContext<'_>, open_path: Option<PathBuf>) -> Self {
        let get = |k: &str| cc.storage.and_then(|s| s.get_string(k));
        let mut app = Self {
            tree: None,
            drive: None,
            zoom: Vec::new(),
            cam: None,
            fit: None,
            view: (0.0, 0.0),
            resize_anchor: None,
            items: Vec::new(),
            layout_key: None,
            generation: 0,
            selection: Selection::default(),
            marquee: None,
            hovered: None,
            hover_since: Instant::now(),
            anim: None,
            show_free: get("show_free").is_none_or(|v| v == "true"),
            dark: get("dark").is_some_and(|v| v == "true"),
            applied_dark: None,
            params: LayoutParams::default(),
            dialog: None,
            scan: None,
            confirm_delete: None,
            error: None,
            title: String::new(),
            font: FontId::proportional(10.0),
        };
        match open_path {
            Some(p) => match scan::drive_for_path(&p) {
                Some(d) => app.start_scan(d, false, &cc.egui_ctx),
                None => app.error = Some(format!("Cannot open {}", p.display())),
            },
            None => app.open_dialog(),
        }
        app
    }

    fn palette(&self) -> Palette {
        Palette::new(self.dark)
    }

    fn invalidate(&mut self) {
        self.generation += 1;
    }

    // -- selection helpers ---------------------------------------------------

    fn selected_entry(&self) -> Option<&model::Entry> {
        self.tree.as_ref()?.entry(self.selection.primary()?)
    }

    fn selected_path(&self) -> Option<PathBuf> {
        Some(self.tree.as_ref()?.path_of(self.selection.primary()?))
    }

    fn selected_item(&self) -> Option<usize> {
        let r = self.selection.primary()?;
        self.items.iter().position(|it| it.index == Some(r.index) && it.folder == r.folder)
    }

    fn item_ref(&self, idx: usize) -> Option<EntryRef> {
        let it = &self.items[idx];
        Some(EntryRef::new(it.folder.clone(), it.index?))
    }

    /// Boxes of the folder's entries as drawn now (natural split of its content area at the
    /// current camera), without free space.
    fn sibling_boxes(&self, folder: &[usize]) -> Vec<(usize, Rect)> {
        let (Some(t), Some(cam)) = (&self.tree, self.cam) else { return Vec::new() };
        let p = self.params();
        let Some(f) = t.folder_at(folder) else { return Vec::new() };
        let Some(area) = layout::content_of(&t.root, cam, folder, p, &self.ovs_at(cam)) else { return Vec::new() };
        layout::child_boxes(f, area, p)
            .into_iter()
            .filter(|&(i, _)| !matches!(f.entries[i].kind, model::Kind::Free))
            .collect()
    }

    /// Arrow keys: move the selection (see `Selection::navigate`), then pan so the primary
    /// selection is in view.
    fn navigate(&mut self, nav: Nav) {
        self.finish_anim();
        let mut sel = std::mem::take(&mut self.selection);
        sel.navigate(nav, |f| self.sibling_boxes(f));
        self.selection = sel;
        self.reveal_primary();
    }

    /// Pan the least amount that brings the primary selection's box fully into view (or, if
    /// it's bigger than the view, lines its top / left edge up with the view's).
    fn reveal_primary(&mut self) {
        let (Some(s), Some(cam)) = (self.selection.primary(), self.cam) else { return };
        let Some(b) = self.locate(cam, &s.path(), &self.ovs_at(cam)) else { return };
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
            self.pan(dx, dy);
        }
    }

    // -- commands ------------------------------------------------------------

    fn open_dialog(&mut self) {
        self.dialog = Some(DriveDialog { drives: scan::volumes(), selected: None, path: String::new() });
    }

    fn start_scan(&mut self, drive: Drive, is_drive: bool, ctx: &egui::Context) {
        if !is_drive {
            self.show_free = false;
        }
        if let Some(job) = self.scan.take() {
            job.ctl.cancel();
        }
        self.tree = None;
        self.zoom.clear();
        self.cam = None;
        self.fit = None;
        self.selection.clear();
        self.marquee = None;
        self.anim = None;
        self.invalidate();

        let ctl = Arc::new(ScanControl::default());
        let (tx, rx) = mpsc::channel();
        let (d, c, ctx) = (drive.clone(), ctl.clone(), ctx.clone());
        std::thread::spawn(move || {
            let res = scan::scan(&d, &c);
            let _ = tx.send(res);
            ctx.request_repaint();
        });
        self.drive = Some(drive);
        self.scan = Some(ScanJob { drive: self.drive.clone().unwrap(), ctl, rx });
    }

    fn poll_scan(&mut self) {
        let Some(job) = &self.scan else { return };
        match job.rx.try_recv() {
            Ok(Some(tree)) => {
                self.tree = Some(tree);
                self.scan = None;
                self.invalidate();
            }
            Ok(None) | Err(mpsc::TryRecvError::Disconnected) => self.scan = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn params(&self) -> LayoutParams {
        LayoutParams { show_free: self.show_free, ..self.params }
    }

    fn full_view(&self) -> Rect {
        Rect::new(0.0, 0.0, self.view.0, self.view.1)
    }

    /// Box a folder needs so its content exactly fills the view.
    fn fill_box(&self) -> Rect {
        Rect::new(-3.0, -12.0, self.view.0 + 5.0, self.view.1 + 14.0)
    }

    fn scale_of(&self, c: Rect) -> f64 {
        c.w / self.view.0.max(1.0)
    }

    fn zoomed(&self) -> bool {
        self.fit.is_some() || self.cam.is_some_and(|c| c != self.full_view())
    }

    /// Overrides in effect for camera `c` (outside animations).
    fn ovs_at(&self, c: Rect) -> Vec<Reshape> {
        match &self.fit {
            Some(f) => {
                let b = f.blend(self.scale_of(c));
                if b > 0.0 { vec![f.ov.faded(b)] } else { Vec::new() }
            }
            None => Vec::new(),
        }
    }

    /// Camera and overrides to draw this frame.
    fn view_state(&self) -> (Rect, Vec<Reshape>) {
        let cam = self.cam.unwrap_or(self.full_view());
        match &self.anim {
            Some(a) => self.anim_state(a),
            None => (cam, self.ovs_at(cam)),
        }
    }

    fn locate(&self, cam: Rect, path: &[usize], ovs: &[Reshape]) -> Option<Rect> {
        layout::locate(&self.tree.as_ref()?.root, cam, path, self.params(), ovs)
    }

    /// Scale camera `c` by `k` about point `p`.
    fn scaled(c: Rect, p: (f64, f64), k: f64) -> Rect {
        Rect::new(p.0 + (c.x - p.0) * k, p.1 + (c.y - p.1) * k, c.w * k, c.h * k)
    }

    /// Keep the root covering the view (no zooming out past the scan root), and while a
    /// folder is fully fitted, keep it covering the view too.
    fn clamp_cam(&self, mut c: Rect) -> Rect {
        let (vw, vh) = self.view;
        if c.w < vw || c.h < vh {
            return self.full_view();
        }
        // Allowed shifts: keep the root covering the view...
        let (mut xlo, mut xhi) = (vw - c.w - c.x, -c.x);
        let (mut ylo, mut yhi) = (vh - c.h - c.y, -c.y);
        // ...and a fully fitted folder's content too.
        if let Some(f) = self.fit.as_ref().filter(|f| f.blend(self.scale_of(c)) >= 1.0) {
            if let Some(b) = self.locate(c, &f.ov.path, std::slice::from_ref(&f.ov)) {
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
    fn cover_root(&self, mut c: Rect) -> Rect {
        let (vw, vh) = self.view;
        if c.w < vw || c.h < vh {
            return self.full_view();
        }
        c.x = c.x.min(0.0).max(vw - c.w);
        c.y = c.y.min(0.0).max(vh - c.h);
        c
    }

    /// Even-zoom camera that centres the bounding box of the entries at `paths` (natural boxes)
    /// and fits it in the view with a margin, as far as the root allows.
    fn solve_frame(&self, paths: &[Vec<usize>]) -> Option<Rect> {
        let (vw, vh) = self.view;
        let bbox = |cam: Rect| {
            let mut bb: Option<(f64, f64, f64, f64)> = None;
            for r in paths.iter().filter_map(|p| self.locate(cam, p, &[])) {
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

    /// Frame the selection (F): a single folder fills the view as with Zoom In; anything else
    /// is framed by its bounding box. Nothing selected frames the whole map. The selection stays.
    fn frame_selection(&mut self) {
        let roots = self.selection.roots();
        let folder = |r: &EntryRef| self.tree.as_ref().and_then(|t| t.entry(r)).is_some_and(|e| e.child().is_some());
        match roots.as_slice() {
            [] => self.zoom_to(&[]),
            [r] if folder(r) => self.zoom_to(&r.path()),
            _ => {
                self.finish_anim();
                let paths: Vec<Vec<usize>> = roots.iter().map(EntryRef::path).collect();
                let (Some(cam0), Some(end_cam)) = (self.cam, self.solve_frame(&paths)) else { return };
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
        }
        if let Some(a) = &mut self.anim {
            a.keep_sel = true;
        }
    }

    /// Even-zoom camera at which the folder's natural box has the window's area and sits in
    /// the middle (as far as the root allows), plus the reshape that makes it fill the window.
    fn solve_fit(&self, path: &[usize]) -> Option<(Rect, Fit)> {
        let t = self.fill_box();
        let mut cam = self.cam?;
        for _ in 0..40 {
            let n = self.locate(cam, path, &[])?;
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
        let n = self.locate(cam, path, &[])?;
        let ov = Reshape::between(path.to_vec(), n, t);
        Some((cam, Fit { ov, scale: self.scale_of(cam) }))
    }

    /// Animate to the folder at `path`, filling the view.
    fn zoom_to(&mut self, path: &[usize]) {
        self.finish_anim();
        let Some(cam0) = self.cam else { return };
        let ovs0 = self.ovs_at(cam0);
        let Some(b0) = self.locate(cam0, path, &ovs0) else { return };
        // Any other reshaped folder fades out on the way.
        let old: Option<Reshape> = ovs0.iter().find(|o| o.path != path).cloned();
        let Some(n0) = self.locate(cam0, path, old.as_slice()) else { return };

        let (end_cam, end_fit) = if path.is_empty() {
            (self.full_view(), None)
        } else {
            let Some((c, f)) = self.solve_fit(path) else { return };
            (c, Some(f))
        };
        let Some(n1) = self.locate(end_cam, path, &[]) else { return };
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

    /// Camera and overrides partway through an animation.
    fn anim_state(&self, a: &Anim) -> (Rect, Vec<Reshape>) {
        let (vw, vh) = self.view;
        let t = (a.start.elapsed().as_secs_f32() / ANIM_DURATION).min(1.0) as f64;
        let e = ease_out_cubic(t);
        let (c0, c1) = (a.from_cam, a.end_cam);

        // Even zoom about the fixed point of the whole move (a plain pan if there's no zoom).
        let k = c1.w / c0.w;
        let fp = |q: (f64, f64), e: f64| move_point(c0, c1, q, e);
        let (x, y) = fp((c0.x, c0.y), e);
        let r = k.powf(e);
        let mut cam = Rect::new(x, y, c0.w * r, c0.h * r);

        // Another folder's reshape fades out; that moves things inside it, so keep the target's
        // natural centre on its straight path regardless.
        let mut ovs: Vec<Reshape> = a.old.iter().map(|o| o.faded(1.0 - e)).collect();
        if !ovs.is_empty() {
            let (end, want) = (fp(a.n0, 1.0), fp(a.n0, e));
            let want = (want.0 + (a.n1.0 - end.0) * e, want.1 + (a.n1.1 - end.1) * e);
            if let Some(n) = self.locate(cam, &a.target, &ovs) {
                let nc = n.center();
                cam.x = (cam.x + want.0 - nc.0).min(0.0).max(vw - cam.w);
                cam.y = (cam.y + want.1 - nc.1).min(0.0).max(vh - cam.h);
            }
        }

        // The target's box changes shape evenly between its start and end reshape.
        if !a.target.is_empty() {
            ovs.push(Reshape {
                path: a.target.clone(),
                a: (geo_lerp(a.ov0.a.0, a.ov1.a.0, e), geo_lerp(a.ov0.a.1, a.ov1.a.1, e)),
                d: (lerp(a.ov0.d.0, a.ov1.d.0, e), lerp(a.ov0.d.1, a.ov1.d.1, e)),
            });
        }
        (cam, ovs)
    }

    fn finish_anim(&mut self) {
        if let Some(a) = self.anim.take() {
            self.cam = Some(a.end_cam);
            self.fit = a.end_fit;
            if !a.keep_sel {
                self.selection.clear();
            }
        }
    }

    fn step_anim(&mut self) {
        if self.anim.as_ref().is_some_and(|a| a.start.elapsed().as_secs_f32() >= ANIM_DURATION) {
            self.finish_anim();
        }
    }

    fn zoom_in_item(&mut self, idx: usize) {
        let it = &self.items[idx];
        let Some(target) = it.path().filter(|_| it.is_folder) else { return };
        self.zoom_to(&target);
    }

    /// Zoom out: fit the current folder if it isn't already, otherwise its parent.
    fn zoom_out(&mut self) {
        let Some(cam) = self.cam else { return };
        let view = layout::root_content(self.full_view());
        let ovs = self.ovs_at(cam);
        let fitted = match self.tree.as_ref() {
            Some(t) => layout::content_of(&t.root, cam, &self.zoom, self.params(), &ovs)
                .is_some_and(|c| (c.x - view.x).abs() < 0.5 && (c.y - view.y).abs() < 0.5 && (c.w - view.w).abs() < 0.5 && (c.h - view.h).abs() < 0.5),
            None => return,
        };
        let mut target = self.zoom.clone();
        if fitted {
            target.pop();
        }
        self.zoom_to(&target);
    }

    /// Scale the camera by `k` about a view point, keeping whatever is under it in place.
    fn zoom_at(&mut self, px: f64, py: f64, k: f64) {
        let Some(cam) = self.cam else { return };
        let (vw, vh) = self.view;
        let k = k.min(MAX_ZOOM * vw / cam.w).min(MAX_ZOOM * vh / cam.h);

        // Deepest entry under the point (drawn last).
        let anchor = self
            .items
            .iter()
            .rev()
            .find(|it| it.index.is_some() && !it.is_free && it.contains(px as f32, py as f32))
            .and_then(Item::path);
        let before = anchor.and_then(|path| self.locate(cam, &path, &self.ovs_at(cam)).map(|b| (path, b)));

        let mut c = Self::scaled(cam, (px, py), k);
        // Frames don't scale (and a fitted folder's stretch fades), so pin the anchor.
        if let Some((path, b0)) = before.filter(|(_, b)| b.w > 1e-6 && b.h > 1e-6) {
            if let Some(b1) = self.locate(c, &path, &self.ovs_at(c)) {
                let (u, v) = ((px - b0.x) / b0.w, (py - b0.y) / b0.h);
                c.x += px - (b1.x + u * b1.w);
                c.y += py - (b1.y + v * b1.h);
            }
        }
        self.cam = Some(self.clamp_cam(c));
        self.drop_faded_fit();
    }

    fn drop_faded_fit(&mut self) {
        if let (Some(f), Some(c)) = (&self.fit, self.cam) {
            if f.blend(self.scale_of(c)) <= 0.0 {
                self.fit = None;
            }
        }
    }

    fn pan(&mut self, dx: f64, dy: f64) {
        if let Some(c) = self.cam {
            self.cam = Some(self.clamp_cam(Rect::new(c.x + dx, c.y + dy, c.w, c.h)));
        }
    }

    /// Deepest folder at the window centre that covers a good share of it.
    fn pick_anchor(&self) -> Option<Anchor> {
        let cam = self.cam?;
        let (vw, vh) = self.view;
        let (cx, cy) = (vw / 2.0, vh / 2.0);
        let ovs = self.ovs_at(cam);
        let it = self
            .items
            .iter()
            .filter(|it| it.is_folder && it.labeled && it.index.is_some() && it.contains(cx as f32, cy as f32))
            .filter(|it| {
                let w = (it.x + it.w).min(vw as f32) - it.x.max(0.0);
                let h = (it.y + it.h).min(vh as f32) - it.y.max(0.0);
                (w.max(0.0) * h.max(0.0)) as f64 >= ANCHOR_MIN_SHARE * vw * vh
            })
            .max_by_key(|it| it.folder.len())?;
        let path = it.path()?;
        let b = self.locate(cam, &path, &ovs)?;
        let (bx, by) = b.center();
        Some(Anchor { path, share: b.area() / (vw * vh), u: bx / vw, v: by / vh })
    }

    /// The view changed size: refit, and hold the anchor folder's share and position.
    fn resized(&mut self, vw: f64, vh: f64) {
        self.finish_anim();
        let (ow, oh) = self.view;
        let Some(cam) = self.cam.filter(|_| ow > 0.0 && oh > 0.0) else {
            self.view = (vw, vh);
            self.cam = None;
            return;
        };
        let was_zoomed = self.zoomed();
        let anchor = match &self.resize_anchor {
            Some((a, at)) if at.elapsed() < RESIZE_SETTLE => a.clone(),
            _ => self.pick_anchor(),
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
            let path = f.ov.path.clone();
            self.fit = self.solve_fit(&path).map(|(_, f)| f);
        }
        if let Some(a) = anchor {
            for _ in 0..12 {
                let Some(b) = self.locate(c, &a.path, &self.ovs_at(c)) else { break };
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
        self.cam = Some(self.clamp_cam(c));
        self.drop_faded_fit();
    }

    fn run(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Open => self.open_dialog(),
            Action::Reload => {
                if let Some(d) = self.drive.clone() {
                    let show_free = self.show_free;
                    self.start_scan(d, true, ctx);
                    self.show_free = show_free;
                }
            }
            Action::ZoomFull => self.zoom_to(&[]),
            Action::ZoomOut => self.zoom_out(),
            Action::ZoomIn => {
                if let Some(idx) = self.selected_item() {
                    self.zoom_in_item(idx);
                }
            }
            Action::ToggleFree => {
                self.show_free = !self.show_free;
                self.invalidate();
            }
            Action::RunOpen => {
                let Some(t) = &self.tree else { return };
                let mut roots = self.selection.roots();
                roots.reverse();
                for r in roots {
                    let p = t.path_of(&r);
                    if let Err(e) = open::that_detached(&p) {
                        self.error = Some(format!("Cannot open {}:\n{e}", p.display()));
                        break;
                    }
                }
            }
            Action::Delete => {
                let Some(t) = &self.tree else { return };
                let list: Vec<(EntryRef, PathBuf)> = self
                    .selection
                    .roots()
                    .into_iter()
                    .map(|r| {
                        let p = t.path_of(&r);
                        (r, p)
                    })
                    .collect();
                if !list.is_empty() {
                    self.confirm_delete = Some(list);
                }
            }
            Action::Hide => {
                let roots = self.selection.roots();
                if let (false, Some(t)) = (roots.is_empty(), &mut self.tree) {
                    for r in &roots {
                        t.hide(&r.folder, r.index);
                    }
                    self.selection.clear();
                    self.invalidate();
                }
            }
            Action::UnhideAll => {
                if let Some(t) = &mut self.tree {
                    t.unhide_all();
                    self.invalidate();
                }
            }
            Action::ToggleDark => self.dark = !self.dark,
            Action::ClearSelection => self.selection.clear(),
            Action::Frame => self.frame_selection(),
            Action::Nav(n) => self.navigate(n),
        }
    }

    /// `list` comes from `Selection::roots`, so removing in order never shifts a later entry.
    fn delete_confirmed(&mut self, list: Vec<(EntryRef, PathBuf)>) {
        let mut errors = Vec::new();
        for (sel, path) in list {
            match trash::delete(&path) {
                Ok(()) => {
                    if let Some(t) = &mut self.tree {
                        t.remove(&sel.folder, sel.index);
                    }
                }
                Err(e) => errors.push(format!("{}:\n{e}", path.display())),
            }
        }
        self.selection.clear();
        self.invalidate();
        if !errors.is_empty() {
            self.error = Some(format!("Failed to move to trash:\n\n{}", errors.join("\n\n")));
        }
    }

    // -- title ---------------------------------------------------------------

    fn compute_title(&self) -> String {
        let Some(t) = &self.tree else { return APP_NAME.to_string() };
        if self.selection.len() > 1 {
            let roots = self.selection.roots();
            let size: u64 = roots.iter().filter_map(|r| t.entry(r)).map(|e| e.size).sum();
            return format!(
                "{} items selected  -  {}  -  {}  -  {APP_NAME}",
                roots.len(),
                format::size_string(size, t.total_space, true),
                format::size_string(size, t.total_space, false),
            );
        }
        if let (Some(e), Some(p)) = (self.selected_entry(), self.selected_path()) {
            return format!(
                "{}  -  {}  -  {}  -  {APP_NAME}",
                p.display(),
                format::size_string(e.size, t.total_space, true),
                format::size_string(e.size, t.total_space, false),
            );
        }
        let size = if self.zoom.is_empty() {
            t.total_space
        } else {
            t.folder_at(&self.zoom).map_or(0, |f| f.total)
        };
        format!(
            "{}  -  {} Total  -  {} Free  -  {APP_NAME}",
            t.full_path(&self.zoom, None).display(),
            format::size_string(size, t.total_space, false),
            format::size_string(t.free_space, t.free_space, false),
        )
    }

    // -- toolbar -------------------------------------------------------------

    fn toolbar(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let has_tree = self.tree.is_some();
        let zoomed = self.zoomed();
        let sel_folder = self.selected_entry().is_some_and(|e| e.child().is_some());
        let has_sel = !self.selection.is_empty();
        let show_free = self.show_free;
        let hidden = self.tree.as_ref().map_or(0, |t| t.hidden_count);
        let dark = self.dark;
        ui.horizontal(|ui| {
            let mut act = None;
            let mut btn = |ui: &mut egui::Ui, enabled: bool, b: egui::Button, a: Action| {
                if ui.add_enabled(enabled, b).clicked() {
                    act = Some(a);
                }
            };
            let b = egui::Button::new;
            btn(ui, true, b("📂 Open"), Action::Open);
            btn(ui, has_tree, b("⟳ Reload"), Action::Reload);
            ui.separator();
            btn(ui, has_tree && zoomed, b("⏏ Zoom Full"), Action::ZoomFull);
            btn(ui, has_tree && sel_folder, b("➕ Zoom In"), Action::ZoomIn);
            btn(ui, has_tree && zoomed, b("➖ Zoom Out"), Action::ZoomOut);
            ui.separator();
            btn(ui, has_tree, b("Free Space").selected(show_free), Action::ToggleFree);
            ui.separator();
            btn(ui, has_sel, b("▶ Run or Open"), Action::RunOpen);
            btn(ui, has_sel, b("🗑 Delete"), Action::Delete);
            btn(ui, has_sel, b("Hide"), Action::Hide);
            let unhide = if hidden > 0 { format!("Unhide All ({hidden})") } else { "Unhide All".to_string() };
            btn(ui, hidden > 0, egui::Button::new(unhide), Action::UnhideAll);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                btn(ui, true, b(if dark { "☀" } else { "🌙" }), Action::ToggleDark);
            });
            act
        })
        .inner
    }

    // -- treemap -------------------------------------------------------------

    /// Path bar: one treemap-style box per folder from the scan root down to the current view.
    /// Returns the zoom path to go to when a parent is clicked.
    fn path_bar(&mut self, ui: &mut egui::Ui) -> Option<Vec<usize>> {
        let tree = self.tree.as_ref()?;
        let pal = self.palette();

        // (label, zoom path, colour). Folders keep the colour they have in the treemap.
        let mut segs = vec![(tree.root_path.display().to_string(), Vec::new(), pal.background)];
        let mut f = &tree.root;
        for (k, &i) in self.zoom.iter().enumerate() {
            let Some(e) = f.entries.get(i) else { break };
            segs.push((e.name.clone(), self.zoom[..=k].to_vec(), pal.depth(k as i32)));
            match e.child() {
                Some(c) => f = c,
                None => break,
            }
        }

        let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), BAR_H as f32), Sense::click());
        let ppp = ui.ctx().pixels_per_point();
        let origin = Pos2::new((resp.rect.min.x * ppp).round() / ppp, (resp.rect.min.y * ppp).round() / ppp);
        let d = Draw { painter: &painter, origin, ppp, pal, font: self.font.clone() };
        let w = resp.rect.width() as i32;
        d.fill(pal.background, 0.0, 0.0, w as f32, BAR_H as f32);

        let pad = 12;
        let widths: Vec<i32> = segs.iter().map(|(l, _, _)| d.text_width(l).0 as i32 + pad).collect();
        let ellipsis_w = d.text_width("…").0 as i32 + pad;
        // Too long? Drop parents from the left (behind a "…" box), always keeping the current folder.
        let mut first = 0;
        while first + 1 < segs.len() {
            let used: i32 = widths[first..].iter().sum::<i32>() + if first > 0 { ellipsis_w } else { 0 };
            if used <= w {
                break;
            }
            first += 1;
        }

        // Lay out boxes: (x, width, segment index or None for the ellipsis).
        let mut boxes = Vec::new();
        let mut x = 0;
        if first > 0 {
            boxes.push((x, ellipsis_w, None));
            x += ellipsis_w;
        }
        for (i, &sw) in widths.iter().enumerate().skip(first) {
            // The current folder takes the rest of the bar, like a treemap row.
            let sw = if i + 1 == segs.len() { (w - x).max(sw) } else { sw };
            boxes.push((x, sw, Some(i)));
            x += sw;
        }

        let pointer = resp.hover_pos().map(|p| (p.x - origin.x) as i32);
        let last = segs.len() - 1;
        let mut clicked = None;
        for &(bx, bw, seg) in &boxes {
            let hovered = seg.is_some_and(|i| i < last) && pointer.is_some_and(|px| px >= bx && px < bx + bw);
            let (label, color) = match seg {
                Some(i) => (segs[i].0.as_str(), segs[i].2),
                None => ("…", pal.background),
            };
            d.cell(bx as f32, 0.0, bw as f32, BAR_H as f32, color, hovered, label);
            if hovered && resp.clicked() {
                clicked = seg.map(|i| segs[i].1.clone());
            }
        }
        if boxes.iter().any(|&(bx, bw, seg)| seg.is_some_and(|i| i < last) && pointer.is_some_and(|px| px >= bx && px < bx + bw)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        clicked
    }

    fn treemap(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        let origin = Pos2::new((resp.rect.min.x * ppp).round() / ppp, (resp.rect.min.y * ppp).round() / ppp);
        let (w, h) = (resp.rect.width(), resp.rect.height());
        let local = |p: Pos2| (p.x - origin.x, p.y - origin.y);

        // Camera: start fully zoomed out; on resize, hold the folder in view steady.
        let (vw, vh) = (w as f64, h as f64);
        if (vw, vh) != self.view {
            self.resized(vw, vh);
        }
        if self.cam.is_none() {
            self.cam = Some(self.full_view());
        }
        self.step_anim();

        // Wheel / pinch: zoom about the pointer, like an infinite canvas.
        if resp.hovered() && self.anim.is_none() && self.marquee.is_none() && self.tree.is_some() {
            let (dy, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let k = pinch as f64 * (dy as f64 * WHEEL_ZOOM).exp();
            if (k - 1.0).abs() > 1e-6 {
                if let Some(p) = resp.hover_pos() {
                    let (x, y) = local(p);
                    self.zoom_at(x as f64, y as f64, k);
                }
            }
        }
        let mods = ui.input(|i| i.modifiers);
        // Shift+drag (left button): rectangle select; with Ctrl too, add to the selection.
        if self.tree.is_some() && resp.drag_started_by(egui::PointerButton::Primary) && mods.shift {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                self.finish_anim();
                let start = local(p);
                let base = if mods.command { self.selection.clone() } else { Selection::default() };
                self.marquee = Some(Marquee::new(start, base));
            }
        }
        if self.marquee.is_some() && !resp.dragged_by(egui::PointerButton::Primary) && !resp.drag_stopped() {
            self.marquee = None;
        }
        // Drag (left or middle button) to pan.
        if self.tree.is_some()
            && self.marquee.is_none()
            && (resp.dragged_by(egui::PointerButton::Primary) || resp.dragged_by(egui::PointerButton::Middle))
        {
            self.finish_anim();
            let d = resp.drag_delta();
            self.pan(d.x as f64, d.y as f64);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        // Rebuild layout when the view, data, camera or reshaping changes.
        let (cam, ovs) = self.view_state();
        let key = (w, h, self.generation, cam, ovs);
        if self.layout_key.as_ref() != Some(&key) {
            let p = self.params();
            self.items = match &self.tree {
                Some(t) => layout::build(&t.root, cam, vw, vh, p, &key.4),
                None => Vec::new(),
            };
            self.zoom = layout::covering(&self.items, vw, vh);
            self.layout_key = Some(key);
            self.hovered = None;
        }

        // Live rectangle selection against this frame's layout.
        if let Some(m) = &self.marquee {
            let end = resp.interact_pointer_pos().map(local).unwrap_or(m.end);
            self.selection = m.selection(&self.items, end);
            if let Some(m) = &mut self.marquee {
                m.end = end;
            }
            if resp.drag_stopped() {
                self.marquee = None;
            }
        }

        let mut act = None;

        // Pointer / hover handling.
        let hit = resp
            .hover_pos()
            .and_then(|p| {
                let (x, y) = local(p);
                layout::hit_test(&self.items, x, y)
            })
            .filter(|_| self.anim.is_none() && !resp.dragged());
        if hit != self.hovered {
            self.hovered = hit;
            self.hover_since = Instant::now();
        }

        let pal = self.palette();
        let d = Draw { painter: &painter, origin, ppp, pal, font: self.font.clone() };

        d.fill(pal.background, 0.0, 0.0, w, h);
        if let Some(tree) = &self.tree {
            let sel: HashSet<&EntryRef> = self.selection.iter().collect();
            // Parents come before children, so a selected folder's children stay visible.
            for (i, it) in self.items.iter().enumerate() {
                let is_sel = it.index.is_some_and(|k| sel.contains(&EntryRef::new(it.folder.clone(), k)));
                d.item(tree, it, is_sel, self.hovered == Some(i));
            }
        }
        if let Some(m) = &self.marquee {
            let (x, y) = (m.start.0.min(m.end.0), m.start.1.min(m.end.1));
            let r = d.rect(x, y, (m.start.0 - m.end.0).abs(), (m.start.1 - m.end.1).abs());
            let c = pal.text;
            painter.rect_filled(r, 0.0, c.gamma_multiply(0.12));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, c), egui::StrokeKind::Inside);
        }

        let pointer_hit = || {
            resp.interact_pointer_pos().and_then(|p| {
                let (x, y) = local(p);
                layout::hit_test(&self.items, x, y)
            })
        };
        if resp.clicked() {
            let hit = pointer_hit().and_then(|i| self.item_ref(i));
            match hit {
                // Ctrl+click toggles, Shift+click adds.
                Some(r) if mods.command => self.selection.toggle(r),
                Some(r) if mods.shift => self.selection.add(r),
                None if mods.command || mods.shift => {}
                // Plain click on the only selected item deselects it.
                Some(r) if self.selection.is_only(&r) => self.selection.clear(),
                r => self.selection.set(r),
            }
        }
        if resp.secondary_clicked() {
            // Right-clicking inside the selection keeps it, so the menu acts on all of it.
            let r = pointer_hit().and_then(|i| self.item_ref(i));
            if !r.as_ref().is_some_and(|r| self.selection.contains(r)) {
                self.selection.set(r);
            }
        }
        if resp.double_clicked() && !mods.command {
            if let Some(i) = pointer_hit() {
                self.selection.set(self.item_ref(i));
                act = Some(if self.items[i].is_folder { Action::ZoomIn } else { Action::RunOpen });
            }
        }

        let has_sel = !self.selection.is_empty();
        let sel_folder = self.selected_entry().is_some_and(|e| e.child().is_some());
        let zoomed = self.zoomed();
        let show_free = self.show_free;
        let hidden = self.tree.as_ref().map_or(0, |t| t.hidden_count);
        resp.context_menu(|ui| {
            let mut item = |ui: &mut egui::Ui, enabled: bool, label: &str, a: Action| {
                if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                    act = Some(a);
                    ui.close();
                }
            };
            item(ui, sel_folder, "Zoom In", Action::ZoomIn);
            item(ui, zoomed, "Zoom Out", Action::ZoomOut);
            item(ui, zoomed, "Zoom Full", Action::ZoomFull);
            ui.separator();
            item(ui, has_sel, "Run / Open", Action::RunOpen);
            item(ui, has_sel, "Delete", Action::Delete);
            item(ui, has_sel, "Hide (H)", Action::Hide);
            item(ui, hidden > 0, "Unhide All (Shift+H)", Action::UnhideAll);
            ui.separator();
            item(ui, true, "Open Drive...", Action::Open);
            item(ui, true, "Rescan Drive", Action::Reload);
            let label = if show_free { "✔ Show Free Space" } else { "Show Free Space" };
            item(ui, true, label, Action::ToggleFree);
        });

        // Tooltips.
        if let (Some(hi), Some(tree), false) = (self.hovered, &self.tree, resp.context_menu_opened()) {
            let held = self.hover_since.elapsed();
            let it = &self.items[hi];
            if held >= INFOTIP_DELAY {
                if let Some(e) = it.index.and_then(|i| tree.entry_at(&it.folder, i)) {
                    if let Some(pp) = resp.hover_pos() {
                        let screen = ui.ctx().content_rect();
                        // Flip to the other side of the pointer near the right/bottom edges.
                        let pivot = match (pp.x > screen.max.x - 260.0, pp.y > screen.max.y - 90.0) {
                            (false, false) => Align2::LEFT_TOP,
                            (true, false) => Align2::RIGHT_TOP,
                            (false, true) => Align2::LEFT_BOTTOM,
                            (true, true) => Align2::RIGHT_BOTTOM,
                        };
                        let off = Vec2::new(if pivot.x() == egui::Align::Min { 14.0 } else { -6.0 },
                                            if pivot.y() == egui::Align::Min { 18.0 } else { -6.0 });
                        egui::Area::new(Id::new("infotip"))
                            .order(Order::Tooltip)
                            .interactable(false)
                            .pivot(pivot)
                            .fixed_pos(pp + off)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                    ui.label(egui::RichText::new(&e.name).strong());
                                    ui.label(format::file_size(e.actual));
                                    ui.label(format::date(e.mtime));
                                });
                            });
                    }
                }
            } else {
                ui.ctx().request_repaint_after(INFOTIP_DELAY - held);
            }
        }

        if self.anim.is_some() {
            ui.ctx().request_repaint();
        }

        act
    }

    // -- dialogs -------------------------------------------------------------

    fn drive_dialog(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.dialog else { return };
        let mut chosen: Option<(Drive, bool)> = None;
        let mut close = false;
        let mut err = None;

        let modal = egui::Modal::new(Id::new("drive_dialog")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("Select Drive to View");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                if dlg.drives.is_empty() {
                    ui.label("No drives found.");
                }
                for (i, d) in dlg.drives.iter().enumerate() {
                    let used = d.total.saturating_sub(d.free);
                    let text = format!(
                        "{}    {} / {}  ({})    {}",
                        d.name,
                        format::size_string(used, 0, false),
                        format::size_string(d.total, 0, false),
                        format::size_string(used, d.total, true),
                        d.fs
                    );
                    let r = ui.add(egui::Button::selectable(dlg.selected == Some(i), text));
                    if r.clicked() {
                        dlg.selected = Some(i);
                    }
                    if r.double_clicked() {
                        chosen = Some((d.clone(), true));
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Folder:");
                let te = ui.add(egui::TextEdit::singleline(&mut dlg.path).desired_width(240.0));
                let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Browse...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Select Folder").pick_folder() {
                        dlg.path = p.display().to_string();
                    }
                }
                if (ui.add_enabled(!dlg.path.is_empty(), egui::Button::new("Open Folder")).clicked() || enter)
                    && !dlg.path.is_empty()
                {
                    match scan::drive_for_path(&PathBuf::from(dlg.path.trim())) {
                        Some(d) => chosen = Some((d, false)),
                        None => err = Some(format!("Not a readable folder: {}", dlg.path)),
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(dlg.selected.is_some(), egui::Button::new("OK")).clicked() {
                    if let Some(d) = dlg.selected.and_then(|i| dlg.drives.get(i)) {
                        chosen = Some((d.clone(), true));
                    }
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if modal.should_close() {
            close = true;
        }
        if err.is_some() {
            self.error = err;
        }
        if let Some((d, is_drive)) = chosen {
            self.dialog = None;
            self.start_scan(d, is_drive, ctx);
        } else if close {
            self.dialog = None;
        }
    }

    fn scan_dialog(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.scan else { return };
        let ctl = &job.ctl;
        let used = job.drive.total.saturating_sub(job.drive.free);
        let bytes = ctl.bytes.load(Ordering::Relaxed);
        let mut cancel = false;
        egui::Modal::new(Id::new("scan_dialog")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("Scanning Disk...");
            let cur = ctl.current.lock().map(|s| s.clone()).unwrap_or_default();
            ui.label(elide_start(&cur, 70));
            ui.label(format!("Files Found:  {}", ctl.files.load(Ordering::Relaxed)));
            ui.label(format!("Folders Found:  {}", ctl.folders.load(Ordering::Relaxed)));
            let frac = if used > 0 { (bytes as f32 / used as f32).min(1.0) } else { 0.0 };
            ui.add(egui::ProgressBar::new(frac).show_percentage());
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
        if cancel {
            ctl.cancel();
            self.scan = None;
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn message_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(list) = &self.confirm_delete {
            let mut answer = None;
            egui::Modal::new(Id::new("confirm_delete")).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.heading("Delete");
                if let [(_, path)] = list.as_slice() {
                    ui.label(format!("Move to trash?\n\n{}", path.display()));
                } else {
                    ui.label(format!("Move {} items to trash?", list.len()));
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for (_, path) in list.iter().rev() {
                            ui.label(path.display().to_string());
                        }
                    });
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Move to Trash").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                });
            });
            if let Some(yes) = answer {
                let list = self.confirm_delete.take().unwrap_or_default();
                if yes {
                    self.delete_confirmed(list);
                }
            }
        }
        if let Some(msg) = &self.error {
            let mut close = false;
            egui::Modal::new(Id::new("error")).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.heading("Error");
                ui.label(msg);
                if ui.button("OK").clicked() {
                    close = true;
                }
            });
            if close {
                self.error = None;
            }
        }
    }
}

impl eframe::App for SpaceMonger {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.applied_dark != Some(self.dark) {
            self.applied_dark = Some(self.dark);
            ctx.set_visuals(if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        }
        self.poll_scan();

        let mut act = egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui)).inner;
        let pal = self.palette();
        let tm = egui::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(pal.background))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                if let Some(target) = self.path_bar(ui) {
                    self.zoom_to(&target);
                }
                self.treemap(ui)
            })
            .inner;
        act = act.or(tm);

        if self.dialog.is_none() && self.scan.is_none() && self.error.is_none() && self.confirm_delete.is_none() {
            let keys = ctx.input(|i| {
                let extend = i.modifiers.shift || i.modifiers.command;
                let arrow = |d| if extend { Nav::Extend(d) } else { Nav::Sibling(d) };
                if i.key_pressed(egui::Key::Backspace) {
                    Some(Action::ZoomOut)
                } else if i.key_pressed(egui::Key::Enter) {
                    Some(Action::ZoomIn)
                } else if i.key_pressed(egui::Key::Delete) {
                    Some(Action::Delete)
                } else if i.key_pressed(egui::Key::H) && i.modifiers.shift {
                    Some(Action::UnhideAll)
                } else if i.key_pressed(egui::Key::H) {
                    Some(Action::Hide)
                } else if i.key_pressed(egui::Key::F5) {
                    Some(Action::Reload)
                } else if i.key_pressed(egui::Key::F) && !i.modifiers.any() {
                    Some(Action::Frame)
                } else if i.key_pressed(egui::Key::ArrowUp) && i.modifiers.alt {
                    Some(Action::Nav(Nav::Parent))
                } else if i.key_pressed(egui::Key::ArrowDown) && i.modifiers.alt {
                    Some(Action::Nav(Nav::FirstChild))
                } else if i.key_pressed(egui::Key::ArrowUp) {
                    Some(Action::Nav(arrow(crate::core::layout::Dir::Up)))
                } else if i.key_pressed(egui::Key::ArrowDown) {
                    Some(Action::Nav(arrow(crate::core::layout::Dir::Down)))
                } else if i.key_pressed(egui::Key::ArrowLeft) {
                    Some(Action::Nav(arrow(crate::core::layout::Dir::Left)))
                } else if i.key_pressed(egui::Key::ArrowRight) {
                    Some(Action::Nav(arrow(crate::core::layout::Dir::Right)))
                } else if i.key_pressed(egui::Key::Escape) {
                    Some(Action::ClearSelection)
                } else {
                    None
                }
            });
            act = act.or(keys);
        }
        if let Some(a) = act {
            self.run(a, &ctx);
        }

        self.drive_dialog(&ctx);
        self.scan_dialog(&ctx);
        self.message_dialogs(&ctx);

        let title = self.compute_title();
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette().background.to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("dark", self.dark.to_string());
        storage.set_string("show_free", self.show_free.to_string());
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

// ---------------------------------------------------------------------------
// Drawing (port of FolderView.minimalDrawDisplayFolder & friends)

struct Draw<'a> {
    painter: &'a Painter,
    origin: Pos2,
    ppp: f32,
    pal: Palette,
    font: FontId,
}

impl Draw<'_> {
    fn snap(&self, p: Pos2) -> Pos2 {
        Pos2::new((p.x * self.ppp).round() / self.ppp, (p.y * self.ppp).round() / self.ppp)
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> egui::Rect {
        let r = egui::Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h)).translate(self.origin.to_vec2());
        egui::Rect::from_min_max(self.snap(r.min), self.snap(r.max))
    }

    fn fill(&self, c: Color32, x: f32, y: f32, w: f32, h: f32) {
        if w > 0.0 && h > 0.0 {
            self.painter.rect_filled(self.rect(x, y, w, h), 0.0, c);
        }
    }

    fn text_width(&self, s: &str) -> (f32, f32) {
        let g = self.painter.layout_no_wrap(s.to_string(), self.font.clone(), Color32::WHITE);
        (g.size().x.ceil(), g.size().y.ceil())
    }

    fn text(&self, p: &Painter, s: &str, x: f32, y: f32, c: Color32) {
        let pos = self.snap(self.origin + Vec2::new(x, y));
        p.text(pos, Align2::LEFT_TOP, s, self.font.clone(), c);
    }

    /// A path-bar box in the treemap's style: fill, thin border (2px black on hover), label on the left.
    #[allow(clippy::too_many_arguments)]
    fn cell(&self, x: f32, y: f32, w: f32, h: f32, color: Color32, hover: bool, label: &str) {
        let fill = if hover { color.lerp_to_gamma(Color32::WHITE, 0.2) } else { color };
        self.fill(fill, x + 1.0, y + 1.0, w - 1.0, h - 1.0);
        let (bc, bw) = if hover { (Color32::BLACK, 2.0) } else { (self.pal.border, 1.0) };
        self.painter.rect_stroke(
            self.rect(x + 1.0, y + 1.0, w - 1.0, h - 1.0),
            0.0,
            Stroke::new(bw / self.ppp, bc),
            egui::StrokeKind::Inside,
        );
        let (_, th) = self.text_width(label);
        let p = self.painter.with_clip_rect(self.rect(x, y, w, h).intersect(self.painter.clip_rect()));
        self.text(&p, label, x + 6.0, y + 1.0 + (h - 1.0 - th) / 2.0, self.pal.text);
    }

    /// Flat box: a single fill with a 1px gap to its neighbours, plus its label.
    fn item(&self, tree: &Tree, it: &Item, sel: bool, hover: bool) {
        let pal = &self.pal;
        let (x, y, w, h) = (it.x, it.y, it.w + 1.0, it.h + 1.0);

        if !it.is_free {

            let color = if sel {
                pal.text
            } else if hover {
                pal.depth(it.depth).lerp_to_gamma(Color32::WHITE, 0.2)
            } else {
                pal.depth(it.depth)
            };

            
            self.fill(color, x + 1.0, y + 1.0, w - 2.0, h - 2.0);
            
            // draw border
            if w > 4.0 && h > 4.0 {
                
                let border_color = if sel {
                    pal.text
                } else if hover {
                    Color32::BLACK
                } else {
                    pal.border
                };

                let border_width = if hover { 2.0 } else { 1.0};

                // 1 physical pixel dark-grey border, just inside the fill.
                let r = self.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0);
                self.painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(border_width / self.ppp, border_color),
                    egui::StrokeKind::Inside,
                );
            }
        }

        if !it.labeled {
            return;
        }
        let Some(entry) = it.index.and_then(|i| tree.entry_at(&it.folder, i)) else { return };
        let p = self.painter.with_clip_rect(self.rect(x, y, w, h).intersect(self.painter.clip_rect()));
        let fg = if sel { pal.background } else { pal.text };

        if it.is_free {
            let ts = tree.total_space.max(1);
            let fp = tree.free_space as u128 * 1000 / ts as u128;
            let lines = [
                format!("<Free Space: {}.{}%>", fp / 10, fp % 10),
                format!("{} Free", format::size_string(tree.free_space, tree.total_space, false)),
                format!("Files Total:  {}", tree.num_files),
                format!("Folders Total:  {}", tree.num_folders),
            ];
            let (lw, lh) = self.text_width(&lines[0]);
            let tx = if lw > w - 2.0 { x + 2.0 } else { x + (w - lw) / 2.0 };
            let ty = if lh > h - 2.0 { y + 1.0 } else { y + (h - lh) / 2.0 };
            for (line, dy) in lines.iter().zip([-18.0, -6.0, 6.0, 15.0]) {
                self.text(&p, line, tx, ty + dy, pal.text);
            }
            return;
        }

        let (tw, th) = self.text_width(&entry.name);
        let tx = if tw > w - 2.0 || it.is_folder { x + 3.0 } else { x + (w - tw) / 2.0 };
        let mut ty = if th > h - 2.0 || it.is_folder { y + 2.0 } else { y + (h - th) / 2.0 };

        if !it.is_folder && h >= 36.0 && w >= 48.0 {
            for (s, dy) in [(format::file_size(entry.actual), 1.0), (format::date(entry.mtime), 11.0)] {
                let (sw, _) = self.text_width(&s);
                let sx = if sw > w - 2.0 { x + 3.0 } else { x + (w - sw) / 2.0 };
                self.text(&p, &s, sx, ty + dy, fg);
            }
            ty -= 12.0;
        }
        self.text(&p, &entry.name, tx, ty, fg);
    }

}
