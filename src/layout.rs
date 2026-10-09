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
//! An `Ov` reshapes one folder's box (used to make a zoomed-in folder fill the window);
//! that folder is drawn after everything else so it sits on top of its neighbours.

use crate::core::model::{Entry, Folder, Kind};
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

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn area(&self) -> f64 {
        self.w.max(0.0) * self.h.max(0.0)
    }

    /// Does `self` contain `o` (with half a point of slack)?
    pub fn covers(&self, o: &R) -> bool {
        self.x <= o.x + 0.5 && self.y <= o.y + 0.5 && self.x + self.w >= o.x + o.w - 0.5 && self.y + self.h >= o.y + o.h - 0.5
    }
}

/// Reshapes the box of the entry at `path`: scaled per axis by `a` about its centre, with the
/// centre moved by `d` (in units of the natural box size). Both are relative to the natural
/// box, so the override zooms and pans with the camera.
#[derive(Clone, Debug, PartialEq)]
pub struct Ov {
    pub path: Vec<usize>,
    pub a: (f64, f64),
    pub d: (f64, f64),
}

impl Ov {
    pub fn apply(&self, n: R) -> R {
        let (w, h) = (n.w * self.a.0, n.h * self.a.1);
        let cx = n.x + n.w * (0.5 + self.d.0);
        let cy = n.y + n.h * (0.5 + self.d.1);
        R::new(cx - w / 2.0, cy - h / 2.0, w, h)
    }

    /// The override that turns natural box `n` into box `t`.
    pub fn between(path: Vec<usize>, n: R, t: R) -> Self {
        let (nc, tc) = (n.center(), t.center());
        Ov { path, a: (t.w / n.w, t.h / n.h), d: ((tc.0 - nc.0) / n.w, (tc.1 - nc.1) / n.h) }
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

/// A reshaped folder, laid out after everything else so it draws on top.
struct Deferred {
    folder: Rc<[usize]>,
    index: usize,
    depth: i32,
    r: R,
}

struct Ctx<'a> {
    out: &'a mut Vec<Item>,
    ovs: &'a [Ov],
    deferred: std::collections::VecDeque<Deferred>,
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

    /// Does one of `indices` in the folder at `path` lead to a reshaped folder? Its natural box
    /// may be off-screen while the reshaped one isn't, so that branch is never culled.
    fn leads_to_ov(&self, path: &[usize], indices: &[usize]) -> bool {
        self.ovs
            .iter()
            .any(|o| o.path.len() > path.len() && o.path.starts_with(path) && indices.contains(&o.path[path.len()]))
    }

    fn ov_for(&self, path: &[usize], i: usize) -> Option<&Ov> {
        self.ovs
            .iter()
            .find(|o| o.path.len() == path.len() + 1 && o.path.starts_with(path) && o.path[path.len()] == i)
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
/// inside the `vw` x `vh` view. Folders named in `ovs` get reshaped boxes.
pub fn build(root: &Folder, cam: R, vw: f64, vh: f64, p: Params, ovs: &[Ov]) -> Vec<Item> {
    let mut out = Vec::new();
    let (hmin, vmin) = MIN_SIZES[(p.density.clamp(-3, 3) + 3) as usize];
    let mut cx = Ctx { out: &mut out, ovs, deferred: Default::default(), p, hmin, vmin, vw, vh };
    layout_folder(&mut cx, root, Rc::from(Vec::new()), root_content(cam), 0);

    // Reshaped folders (and any reshaped folders inside them) go on top.
    while let Some(df) = cx.deferred.pop_front() {
        let Some(folder) = entry_folder(root, &df.folder) else { continue };
        let e = &folder.entries[df.index];
        let labeled = df.r.w > cx.hmin && df.r.h > cx.vmin;
        if cx.on_screen(df.r) {
            cx.push(&df.folder, Some(df.index), df.depth, (e.child().is_some(), false, labeled), df.r);
        }
        if let (Some(child), true) = (e.child(), labeled) {
            let mut cp = df.folder.to_vec();
            cp.push(df.index);
            layout_folder(&mut cx, child, Rc::from(cp), content(df.r), df.depth + 1);
        }
    }
    out
}

fn entry_folder<'a>(root: &'a Folder, path: &[usize]) -> Option<&'a Folder> {
    let mut f = root;
    for &i in path {
        f = f.entries.get(i)?.child()?;
    }
    Some(f)
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
    if !cx.on_screen(r) && !cx.leads_to_ov(path, indices) {
        return;
    }
    let Some([(l1, r1), (l2, r2)]) = halves(folder, indices, r, cx.p) else { return };
    place(cx, folder, path, &l1, r1, depth);
    place(cx, folder, path, &l2, r2, depth);
}

fn place(cx: &mut Ctx, folder: &Folder, path: &Rc<[usize]>, list: &[usize], r: R, depth: i32) {
    if list.is_empty() || (!cx.on_screen(r) && !cx.leads_to_ov(path, list)) {
        return;
    }
    let labeled = r.w > cx.hmin && r.h > cx.vmin;
    let visible = r.w > MIN_BOX && r.h > MIN_BOX;
    if list.len() > 1 && visible {
        split(cx, folder, path, list, r, depth);
    } else if list.len() == 1 && visible {
        let i = list[0];
        if let Some(ov) = cx.ov_for(path, i) {
            let r = ov.apply(r);
            cx.deferred.push_back(Deferred { folder: path.clone(), index: i, depth, r });
            return;
        }
        if !cx.on_screen(r) {
            return;
        }
        let e = &folder.entries[i];
        let is_free = matches!(e.kind, Kind::Free);
        let d = if is_free { -1 } else { depth };
        cx.push(path, Some(i), d, (e.child().is_some(), is_free, labeled), r);
        if let (Some(child), true) = (e.child(), labeled) {
            let mut cp = path.to_vec();
            cp.push(i);
            layout_folder(cx, child, Rc::from(cp), content(r), depth + 1);
        }
    } else if cx.on_screen(r) {
        // A few pixels across: merge into one anonymous block.
        cx.push(path, None, depth, (false, false, false), r);
    }
}

/// Direction of an arrow-key move.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// Boxes of the folder's laid-out entries (not hidden or empty) when its content area is
/// `area`: the plain split, with no clipping or size limits.
pub fn child_boxes(folder: &Folder, area: R, p: Params) -> Vec<(usize, R)> {
    fn walk(folder: &Folder, list: &[usize], r: R, p: Params, out: &mut Vec<(usize, R)>) {
        let Some(halves) = halves(folder, list, r, p) else { return };
        for (l, r) in halves {
            match l.len() {
                0 => {}
                1 => out.push((l[0], r)),
                _ => walk(folder, &l, r, p, out),
            }
        }
    }
    let mut out = Vec::new();
    let all: Vec<usize> = (0..folder.entries.len()).collect();
    walk(folder, &all, area, p, &mut out);
    out
}

/// The box next to `from` in direction `d`: among boxes entirely on that side and overlapping
/// it across the direction, the closest; on a tie (several smaller neighbours along one edge)
/// the top-most for left/right moves, the left-most for up/down.
pub fn neighbour(boxes: &[(usize, R)], from: R, d: Dir) -> Option<usize> {
    let eps = 1e-6 * (from.w + from.h);
    let overlap = |a0: f64, a1: f64, b0: f64, b1: f64| a1.min(b1) - a0.max(b0);
    let mut best: Option<(f64, f64, usize)> = None;
    for &(i, c) in boxes {
        let (gap, across, pos) = match d {
            Dir::Right => (c.x - (from.x + from.w), overlap(from.y, from.y + from.h, c.y, c.y + c.h), c.y),
            Dir::Left => (from.x - (c.x + c.w), overlap(from.y, from.y + from.h, c.y, c.y + c.h), c.y),
            Dir::Down => (c.y - (from.y + from.h), overlap(from.x, from.x + from.w, c.x, c.x + c.w), c.x),
            Dir::Up => (from.y - (c.y + c.h), overlap(from.x, from.x + from.w, c.x, c.x + c.w), c.x),
        };
        if gap < -eps || across <= eps {
            continue;
        }
        let better = match best {
            None => true,
            Some((g, q, _)) => gap < g - eps || (gap <= g + eps && pos < q),
        };
        if better {
            best = Some((gap, pos, i));
        }
    }
    best.map(|b| b.2)
}

/// Unclipped box of the entry at `path` (from the scan root) with the root's box at `cam`,
/// as drawn with overrides `ovs`. Pure geometry: ignores size thresholds, so it also works for
/// boxes too small to draw. An empty path gives `cam` itself.
pub fn locate(root: &Folder, cam: R, path: &[usize], p: Params, ovs: &[Ov]) -> Option<R> {
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
        if let Some(o) = ovs.iter().find(|o| o.path[..] == path[..=k]) {
            r = o.apply(r);
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
pub fn content_of(root: &Folder, cam: R, path: &[usize], p: Params, ovs: &[Ov]) -> Option<R> {
    if path.is_empty() {
        Some(root_content(cam))
    } else {
        locate(root, cam, path, p, ovs).map(content)
    }
}

/// Deepest folder whose content area covers the whole `vw` x `vh` view.
pub fn covering(items: &[Item], vw: f64, vh: f64) -> Vec<usize> {
    let view = root_content(R::new(0.0, 0.0, vw, vh));
    items
        .iter()
        .filter(|it| it.is_folder && it.labeled && it.index.is_some())
        .filter(|it| content(R::new(it.x as f64, it.y as f64, it.w as f64, it.h as f64)).covers(&view))
        .max_by_key(|it| it.folder.len())
        .map(|it| {
            let mut p = it.folder.to_vec();
            p.extend(it.index);
            p
        })
        .unwrap_or_default()
}

/// Item under the point, ignoring folder content areas, anonymous blocks and free space.
/// Searches from the end, so reshaped folders (drawn last, on top) win over what's beneath.
pub fn hit_test(items: &[Item], px: f32, py: f32) -> Option<usize> {
    // The last item containing the point is the deepest one on top.
    let i = items.iter().rposition(|it| it.contains(px, py))?;
    let it = &items[i];
    let in_content = it.is_folder && it.labeled && !it.on_frame(px, py);
    (it.index.is_some() && !it.is_free && !in_content).then_some(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: u64) -> Entry {
        Entry { name: name.into(), size, actual: size, mtime: 0, kind: Kind::File, hidden: false }
    }

    fn full(w: f64, h: f64) -> R {
        R::new(0.0, 0.0, w, h)
    }

    #[test]
    fn child_boxes_match_layout() {
        let mut entries: Vec<Entry> = (0..9).map(|i| file(&format!("f{i}"), 900 - i * 90)).collect();
        entries[4].hidden = true;
        let f = Folder { entries, total: 0 };
        let p = Params::default();
        let view = full(2001.0, 1201.0);
        let items = build(&f, view, 2001.0, 1201.0, p, &[]);
        let boxes = child_boxes(&f, root_content(view), p);
        assert_eq!(boxes.iter().map(|b| b.0).collect::<Vec<_>>(), items.iter().filter_map(|it| it.index).collect::<Vec<_>>());
        assert!(!boxes.iter().any(|b| b.0 == 4));
    }

    #[test]
    fn neighbour_picks_adjacent_top_left_aligned() {
        // A tall box on the left, two stacked on the right, one wide box below those.
        //  0 | 1
        //    | 2
        //    |----
        //    |  3
        let b = [
            (0, R::new(0.0, 0.0, 10.0, 30.0)),
            (1, R::new(10.0, 0.0, 10.0, 10.0)),
            (2, R::new(10.0, 10.0, 10.0, 10.0)),
            (3, R::new(10.0, 20.0, 10.0, 10.0)),
        ];
        let at = |i: usize| b[i].1;
        assert_eq!(neighbour(&b, at(0), Dir::Right), Some(1)); // larger -> smaller: top-aligned
        assert_eq!(neighbour(&b, at(2), Dir::Left), Some(0));
        assert_eq!(neighbour(&b, at(1), Dir::Down), Some(2));
        assert_eq!(neighbour(&b, at(3), Dir::Up), Some(2));
        assert_eq!(neighbour(&b, at(0), Dir::Left), None);
        assert_eq!(neighbour(&b, at(1), Dir::Up), None);
    }

    #[test]
    fn areas_roughly_proportional() {
        let f = Folder {
            entries: vec![file("a", 600), file("b", 300), file("c", 100)],
            total: 1000,
        };
        let items = build(&f, full(1001.0, 501.0), 1001.0, 501.0, Params::default(), &[]);
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
        let items = build(&f, full(400.0, 300.0), 400.0, 300.0, Params::default(), &[]);
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
        let items = build(&f, cam, 400.0, 300.0, p, &[]);
        for it in items.iter().filter(|it| it.index.is_some()) {
            let mut path = it.folder.to_vec();
            path.push(it.index.unwrap());
            let b = locate(&f, cam, &path, p, &[]).unwrap();
            // Clipped boxes are inside the located (unclipped) one.
            assert!(b.x <= it.x as f64 + 0.01 && b.y <= it.y as f64 + 0.01);
            assert!(b.x + b.w >= (it.x + it.w) as f64 - 0.01);
        }
        assert!(items.iter().all(|it| it.x < 400.0 && it.y < 300.0 && it.x + it.w > 0.0 && it.y + it.h > 0.0));
    }

    #[test]
    fn override_fills_view_and_draws_on_top() {
        let f = nested();
        let p = Params::default();
        let (vw, vh) = (400.0, 300.0);
        let cam = full(vw, vh);
        assert!(covering(&build(&f, cam, vw, vh, p, &[]), vw, vh).is_empty());
        // Reshape "sub" so its content exactly fills the view.
        let n = locate(&f, cam, &[0], p, &[]).unwrap();
        let t = R::new(-3.0, -12.0, vw + 5.0, vh + 14.0);
        let ov = Ov::between(vec![0], n, t);
        let b = locate(&f, cam, &[0], p, std::slice::from_ref(&ov)).unwrap();
        assert!((b.x - t.x).abs() < 1e-9 && (b.w - t.w).abs() < 1e-9 && (b.h - t.h).abs() < 1e-9);
        let items = build(&f, cam, vw, vh, p, std::slice::from_ref(&ov));
        assert_eq!(covering(&items, vw, vh), vec![0]);
        // The reshaped folder and its children come last and win hit tests.
        let x = items.iter().position(|it| it.index == Some(0) && it.folder.len() == 1).unwrap();
        assert_eq!(hit_test(&items, 200.0, 150.0), Some(x));
        let sub = items.iter().position(|it| it.index == Some(0) && it.folder.is_empty()).unwrap();
        assert!(sub > items.iter().position(|it| it.index == Some(1) && it.folder.is_empty()).unwrap());
    }
}
