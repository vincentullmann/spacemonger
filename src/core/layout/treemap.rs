//! Building the visible layout for one frame.

use super::split::halves;
use super::{content, root_content, Item, LayoutParams, Reshape};
use crate::core::geometry::Rect;
use crate::core::model::{Folder, Kind};
use std::rc::Rc;

/// Below this (w or h, in points) entries are merged into one anonymous block.
const MIN_BOX: f64 = 5.0;

/// Items are clipped to the view plus this margin, so huge zoomed-in boxes stay in f32 range
/// and their borders stay off-screen.
const CLIP_MARGIN: f64 = 16.0;

/// A reshaped folder, laid out after everything else so it draws on top.
struct Deferred {
    folder: Rc<[usize]>,
    index: usize,
    depth: i32,
    r: Rect,
}

struct Ctx<'a> {
    out: &'a mut Vec<Item>,
    ovs: &'a [Reshape],
    deferred: std::collections::VecDeque<Deferred>,
    p: LayoutParams,
    hmin: f64,
    vmin: f64,
    vw: f64,
    vh: f64,
}

impl Ctx<'_> {
    fn on_screen(&self, r: Rect) -> bool {
        r.x < self.vw && r.y < self.vh && r.x + r.w > 0.0 && r.y + r.h > 0.0
    }

    /// Does one of `indices` in the folder at `path` lead to a reshaped folder? Its natural box
    /// may be off-screen while the reshaped one isn't, so that branch is never culled.
    fn leads_to_ov(&self, path: &[usize], indices: &[usize]) -> bool {
        self.ovs
            .iter()
            .any(|o| o.path.len() > path.len() && o.path.starts_with(path) && indices.contains(&o.path[path.len()]))
    }

    fn ov_for(&self, path: &[usize], i: usize) -> Option<&Reshape> {
        self.ovs
            .iter()
            .find(|o| o.path.len() == path.len() + 1 && o.path.starts_with(path) && o.path[path.len()] == i)
    }

    fn push(&mut self, folder: &Rc<[usize]>, index: Option<usize>, depth: i32, kind: (bool, bool, bool), r: Rect) {
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
pub fn build(root: &Folder, cam: Rect, vw: f64, vh: f64, p: LayoutParams, ovs: &[Reshape]) -> Vec<Item> {
    let mut out = Vec::new();
    let (hmin, vmin) = p.label_min();
    let mut cx = Ctx { out: &mut out, ovs, deferred: Default::default(), p, hmin, vmin, vw, vh };
    layout_folder(&mut cx, root, Rc::from(Vec::new()), root_content(cam), 0);

    // Reshaped folders (and any reshaped folders inside them) go on top.
    while let Some(df) = cx.deferred.pop_front() {
        let Some(folder) = root.descendant(&df.folder) else { continue };
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

fn layout_folder(cx: &mut Ctx, folder: &Folder, path: Rc<[usize]>, r: Rect, depth: i32) {
    let indices: Vec<usize> = (0..folder.entries.len()).collect();
    split(cx, folder, &path, &indices, r, depth);
}

fn split(cx: &mut Ctx, folder: &Folder, path: &Rc<[usize]>, indices: &[usize], r: Rect, depth: i32) {
    if !cx.on_screen(r) && !cx.leads_to_ov(path, indices) {
        return;
    }
    let Some([(l1, r1), (l2, r2)]) = halves(folder, indices, r, cx.p) else { return };
    place(cx, folder, path, &l1, r1, depth);
    place(cx, folder, path, &l2, r2, depth);
}

fn place(cx: &mut Ctx, folder: &Folder, path: &Rc<[usize]>, list: &[usize], r: Rect, depth: i32) {
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
