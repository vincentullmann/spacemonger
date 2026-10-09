//! Treemap layout (port of FolderView.buildFolderLayout / sizeFolders).
//!
//! Greedy binary split: entries (sorted largest first) are dealt into two
//! lists, keeping their sums as even as possible, and the rectangle is split
//! along its longer side in proportion. Recurse until a list holds one entry
//! or the rectangle is a few pixels wide. Boxes smaller than the density's
//! minimum label size are still real, hoverable items; they just get no label
//! and their folder contents aren't drawn.
//!
//! Geometry is f64 in view coordinates and always starts at the scan root, whose box is the
//! camera. Zooming moves the camera; each frame only the part inside the view is laid out.

use crate::scan::{Entry, Folder, Kind};
use std::rc::Rc;

/// Below this (w or h, in points) entries are merged into one anonymous block.
const MIN_BOX: f64 = 5.0;

/// Minimum (w, h) for a labelled box, by density (-3..=3).
const MIN_SIZES: [(f64, f64); 7] =
    [(96.0, 64.0), (64.0, 48.0), (48.0, 32.0), (32.0, 24.0), (24.0, 16.0), (16.0, 12.0), (8.0, 6.0)];

/// Items are clipped to the view plus this margin, so huge zoomed-in boxes stay in f32 range
/// and their borders stay off-screen.
const CLIP_MARGIN: f64 = 16.0;

#[derive(Clone, Copy)]
pub struct Params {
    pub density: i32,
    /// -20 (prefer vertical splits) .. +20 (prefer horizontal splits).
    pub bias: i32,
    pub show_free: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self { density: 0, bias: 0, show_free: true }
    }
}

/// A rectangle in view coordinates (points). f64 so deep zooms keep sub-pixel precision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct R {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl R {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    /// Does `self` contain `o` (with half a point of slack)?
    pub fn covers(&self, o: &R) -> bool {
        self.x <= o.x + 0.5 && self.y <= o.y + 0.5 && self.x + self.w >= o.x + o.w - 0.5 && self.y + self.h >= o.y + o.h - 0.5
    }
}

/// Content area of a folder box: inside its 3px frame and 12px title bar.
pub fn content(b: R) -> R {
    R::new(b.x + 3.0, b.y + 12.0, b.w - 6.0, b.h - 15.0)
}

/// Content area of the scan root when its box (the camera) is `cam`.
pub fn root_content(cam: R) -> R {
    R::new(cam.x, cam.y, cam.w - 1.0, cam.h - 1.0)
}

pub struct Item {
    /// Index path (from the tree root) of the folder that owns this entry.
    pub folder: Rc<[usize]>,
    /// Entry index within `folder`; `None` for an unlabelled "too small" block.
    pub index: Option<usize>,
    /// Nesting depth from the scan root; -1 for the free-space block.
    pub depth: i32,
    pub is_folder: bool,
    pub is_free: bool,
    /// Big enough to carry a label (and, for folders, to show their contents).
    pub labeled: bool,
    /// Box in view coordinates, clipped to the view (plus a margin).
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Item {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px > self.x && py > self.y && px < self.x + self.w && py < self.y + self.h
    }

    /// For folders: is the point on the frame / title bar (not the content area)?
    pub fn on_frame(&self, px: f32, py: f32) -> bool {
        px < self.x + 3.0 || py < self.y + 12.0 || px > self.x + self.w - 3.0 || py > self.y + self.h - 3.0
    }
}

struct Ctx<'a> {
    out: &'a mut Vec<Item>,
    p: Params,
    hmin: f64,
    vmin: f64,
    vw: f64,
    vh: f64,
}

impl Ctx<'_> {
    fn on_screen(&self, r: R) -> bool {
        r.x < self.vw && r.y < self.vh && r.x + r.w > 0.0 && r.y + r.h > 0.0
    }

    fn push(&mut self, folder: &Rc<[usize]>, index: Option<usize>, depth: i32, kind: (bool, bool, bool), r: R) {
        let m = CLIP_MARGIN;
        let x0 = r.x.max(-m);
        let y0 = r.y.max(-m);
        let x1 = (r.x + r.w).min(self.vw + m);
        let y1 = (r.y + r.h).min(self.vh + m);
        let (is_folder, is_free, labeled) = kind;
        self.out.push(Item {
            folder: folder.clone(),
            index,
            depth,
            is_folder,
            is_free,
            labeled,
            x: x0 as f32,
            y: y0 as f32,
            w: (x1 - x0) as f32,
            h: (y1 - y0) as f32,
        });
    }
}

/// Lay out the tree from `root` with the root's box at `cam`, keeping only what falls
/// inside the `vw` x `vh` view.
pub fn build(root: &Folder, cam: R, vw: f64, vh: f64, p: Params) -> Vec<Item> {
    let mut out = Vec::new();
    let (hmin, vmin) = MIN_SIZES[(p.density.clamp(-3, 3) + 3) as usize];
    let mut cx = Ctx { out: &mut out, p, hmin, vmin, vw, vh };
    layout_folder(&mut cx, root, Rc::from(Vec::new()), root_content(cam), 0);
    out
}

fn weight(e: &Entry, p: Params) -> u64 {
    if e.hidden || (matches!(e.kind, Kind::Free) && !p.show_free) {
        0
    } else {
        e.size
    }
}

/// One greedy split step: deal entries (largest first) into two lists with sums as even as
/// possible, and split the rectangle along its longer side in proportion.
/// Scale-invariant, so `locate` can follow it at any zoom.
fn halves(folder: &Folder, indices: &[usize], r: R, p: Params) -> Option<[(Vec<usize>, R); 2]> {
    let mut l1 = Vec::new();
    let mut l2 = Vec::new();
    let (mut s1, mut s2) = (0u128, 0u128);
    for &i in indices {
        let size = weight(&folder.entries[i], p);
        if size == 0 {
            continue;
        }
        if s1 <= s2 {
            l1.push(i);
            s1 += size as u128;
        } else {
            l2.push(i);
            s2 += size as u128;
        }
    }
    if s1 + s2 == 0 {
        return None;
    }

    let (wbias, hbias) = match p.bias {
        b if b > 0 => ((b + 8) as f64, 8.0),
        b if b < 0 => (8.0, (-b + 8) as f64),
        _ => (8.0, 8.0),
    };
    let f = s1 as f64 / (s1 + s2) as f64;
    let (r1, r2) = if r.w * wbias > r.h * hbias {
        let sp = r.w.max(0.0) * f;
        (R::new(r.x, r.y, sp, r.h), R::new(r.x + sp, r.y, r.w - sp, r.h))
    } else {
        let sp = r.h.max(0.0) * f;
        (R::new(r.x, r.y, r.w, sp), R::new(r.x, r.y + sp, r.w, r.h - sp))
    };
    Some([(l1, r1), (l2, r2)])
}

fn layout_folder(cx: &mut Ctx, folder: &Folder, path: Rc<[usize]>, r: R, depth: i32) {
    let indices: Vec<usize> = (0..folder.entries.len()).collect();
    split(cx, folder, &path, &indices, r, depth);
}

fn split(cx: &mut Ctx, folder: &Folder, path: &Rc<[usize]>, indices: &[usize], r: R, depth: i32) {
    if !cx.on_screen(r) {
        return;
    }
    let Some([(l1, r1), (l2, r2)]) = halves(folder, indices, r, cx.p) else { return };
    place(cx, folder, path, &l1, r1, depth);
    place(cx, folder, path, &l2, r2, depth);
}

fn place(cx: &mut Ctx, folder: &Folder, path: &Rc<[usize]>, list: &[usize], r: R, depth: i32) {
    if list.is_empty() || !cx.on_screen(r) {
        return;
    }
    let labeled = r.w > cx.hmin && r.h > cx.vmin;
    let visible = r.w > MIN_BOX && r.h > MIN_BOX;
    if list.len() > 1 && visible {
        split(cx, folder, path, list, r, depth);
    } else if list.len() == 1 && visible {
        let i = list[0];
        let e = &folder.entries[i];
        let is_free = matches!(e.kind, Kind::Free);
        let d = if is_free { -1 } else { depth };
        cx.push(path, Some(i), d, (e.child().is_some(), is_free, labeled), r);
        if let (Some(child), true) = (e.child(), labeled) {
            let mut cp = path.to_vec();
            cp.push(i);
            layout_folder(cx, child, Rc::from(cp), content(r), depth + 1);
        }
    } else {
        // A few pixels across: merge into one anonymous block.
        cx.push(path, None, depth, (false, false, false), r);
    }
}

/// Unclipped box of the entry at `path` (from the scan root) with the root's box at `cam`.
/// Pure geometry: ignores size thresholds, so it also works for boxes too small to draw.
/// An empty path gives `cam` itself.
pub fn locate(root: &Folder, cam: R, path: &[usize], p: Params) -> Option<R> {
    let mut folder = root;
    let mut area = root_content(cam);
    let mut b = cam;
    for (k, &target) in path.iter().enumerate() {
        weight(folder.entries.get(target)?, p).checked_sub(1)?; // not laid out if empty / hidden
        let mut list: Vec<usize> = (0..folder.entries.len()).collect();
        let mut r = area;
        loop {
            let [(l1, r1), (l2, r2)] = halves(folder, &list, r, p)?;
            if l2.is_empty() && l1.len() == 1 {
                r = r1;
                break;
            }
            (list, r) = if l1.contains(&target) {
                (l1, r1)
            } else if l2.contains(&target) {
                (l2, r2)
            } else {
                return None;
            };
            if list.len() == 1 {
                break;
            }
        }
        b = r;
        if k + 1 < path.len() {
            folder = folder.entries[target].child()?;
            area = content(r);
        }
    }
    Some(b)
}

/// Content area of the folder at `path` with the root's box at `cam`.
pub fn content_of(root: &Folder, cam: R, path: &[usize], p: Params) -> Option<R> {
    if path.is_empty() {
        Some(root_content(cam))
    } else {
        locate(root, cam, path, p).map(content)
    }
}

/// Deepest folder whose content area covers the whole `vw` x `vh` view.
pub fn covering(root: &Folder, cam: R, vw: f64, vh: f64, p: Params) -> Vec<usize> {
    let view = root_content(R::new(0.0, 0.0, vw, vh));
    let mut path = Vec::new();
    let mut folder = root;
    let mut area = root_content(cam);
    'outer: loop {
        let mut list: Vec<usize> = (0..folder.entries.len()).collect();
        let mut r = area;
        loop {
            let Some([(l1, r1), (l2, r2)]) = halves(folder, &list, r, p) else { break 'outer };
            (list, r) = if r1.covers(&view) {
                (l1, r1)
            } else if r2.covers(&view) && !l2.is_empty() {
                (l2, r2)
            } else {
                break 'outer;
            };
            if list.len() == 1 {
                let i = list[0];
                match folder.entries[i].child() {
                    Some(c) if content(r).covers(&view) => {
                        path.push(i);
                        folder = c;
                        area = content(r);
                        continue 'outer;
                    }
                    _ => break 'outer,
                }
            }
        }
    }
    path
}

/// Item under the point, ignoring folder content areas, anonymous blocks and free space.
pub fn hit_test(items: &[Item], px: f32, py: f32) -> Option<usize> {
    let i = items
        .iter()
        .position(|it| it.contains(px, py) && (!it.is_folder || !it.labeled || it.on_frame(px, py)))?;
    let it = &items[i];
    (it.index.is_some() && !it.is_free).then_some(i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::Entry;

    fn file(name: &str, size: u64) -> Entry {
        Entry { name: name.into(), size, actual: size, mtime: 0, kind: Kind::File, hidden: false }
    }

    fn full(w: f64, h: f64) -> R {
        R::new(0.0, 0.0, w, h)
    }

    #[test]
    fn areas_roughly_proportional() {
        let f = Folder {
            entries: vec![file("a", 600), file("b", 300), file("c", 100)],
            total: 1000,
        };
        let items = build(&f, full(1001.0, 501.0), 1001.0, 501.0, Params::default());
        assert_eq!(items.len(), 3);
        let area = |n: usize| {
            let it = items.iter().find(|i| i.index == Some(n)).unwrap();
            (it.w * it.h) as f64
        };
        let total = 1000.0 * 500.0;
        assert!((area(0) / total - 0.6).abs() < 0.02);
        assert!((area(1) / total - 0.3).abs() < 0.02);
        assert!((area(2) / total - 0.1).abs() < 0.02);
        // tiles don't overlap and hit-testing finds them
        let a = items.iter().position(|i| i.index == Some(0)).unwrap();
        let it = &items[a];
        assert_eq!(hit_test(&items, it.x + it.w / 2.0, it.y + it.h / 2.0), Some(a));
    }

    #[test]
    fn small_boxes_are_items_without_labels() {
        let f = Folder {
            entries: vec![file("big", 9000), file("s1", 300), file("s2", 200)],
            total: 9500,
        };
        let items = build(&f, full(400.0, 300.0), 400.0, 300.0, Params::default());
        let s1 = items.iter().position(|i| i.index == Some(1)).expect("small file laid out");
        assert!(!items[s1].labeled);
        let it = &items[s1];
        assert_eq!(hit_test(&items, it.x + it.w / 2.0, it.y + it.h / 2.0), Some(s1));
    }

    fn nested() -> Folder {
        let sub = Folder { entries: vec![file("x", 700), file("y", 300)], total: 1000 };
        Folder {
            entries: vec![
                Entry { name: "sub".into(), size: 1000, actual: 1000, mtime: 0, kind: Kind::Dir(Box::new(sub)), hidden: false },
                file("b", 500),
                file("c", 250),
            ],
            total: 1750,
        }
    }

    #[test]
    fn locate_matches_build_and_culls() {
        let f = nested();
        let p = Params::default();
        // Zoomed in 3x around the middle: some boxes fall outside the view.
        let cam = R::new(-400.0, -300.0, 1200.0, 900.0);
        let items = build(&f, cam, 400.0, 300.0, p);
        for it in items.iter().filter(|it| it.index.is_some()) {
            let mut path = it.folder.to_vec();
            path.push(it.index.unwrap());
            let b = locate(&f, cam, &path, p).unwrap();
            // Clipped boxes are inside the located (unclipped) one.
            assert!(b.x <= it.x as f64 + 0.01 && b.y <= it.y as f64 + 0.01);
            assert!(b.x + b.w >= (it.x + it.w) as f64 - 0.01);
        }
        assert!(items.iter().all(|it| it.x < 400.0 && it.y < 300.0 && it.x + it.w > 0.0 && it.y + it.h > 0.0));
    }

    #[test]
    fn covering_finds_fitted_folder() {
        let f = nested();
        let p = Params::default();
        let (vw, vh) = (400.0, 300.0);
        assert!(covering(&f, full(vw, vh), vw, vh, p).is_empty());
        // Fit "sub"'s content to the view by solving for the camera.
        let view = root_content(full(vw, vh));
        let mut cam = full(vw, vh);
        for _ in 0..20 {
            let c = content_of(&f, cam, &[0], p).unwrap();
            let (sx, sy) = (view.w / c.w, view.h / c.h);
            cam = R::new(view.x + (cam.x - c.x) * sx, view.y + (cam.y - c.y) * sy, cam.w * sx, cam.h * sy);
        }
        let c = content_of(&f, cam, &[0], p).unwrap();
        assert!((c.x - view.x).abs() < 1e-6 && (c.w - view.w).abs() < 1e-6);
        assert_eq!(covering(&f, cam, vw, vh, p), vec![0]);
    }
}
