//! Treemap layout (port of FolderView.buildFolderLayout / sizeFolders).
//!
//! Greedy binary split: entries (sorted largest first) are dealt into two
//! lists, keeping their sums as even as possible, and the rectangle is split
//! along its longer side in proportion. Recurse until a list holds one entry
//! or the rectangle is a few pixels wide. Boxes smaller than the density's
//! minimum label size are still real, hoverable items; they just get no label
//! and their folder contents aren't drawn.

use crate::scan::{Folder, Kind};
use std::rc::Rc;

/// Minimum (w, h) for a labelled box, by density (-3..=3).
/// Below this (w or h, in points) entries are merged into one anonymous block.
const MIN_BOX: i32 = 5;

const MIN_SIZES: [(i32, i32); 7] = [(96, 64), (64, 48), (48, 32), (32, 24), (24, 16), (16, 12), (8, 6)];

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

pub struct Item {
    /// Index path (from the tree root) of the folder that owns this entry.
    pub folder: Rc<[usize]>,
    /// Entry index within `folder`; `None` for an unlabelled "too small" block.
    pub index: Option<usize>,
    /// Nesting depth relative to the view root; -1 for the free-space block.
    pub depth: i32,
    pub is_folder: bool,
    pub is_free: bool,
    /// Big enough to carry a label (and, for folders, to show their contents).
    pub labeled: bool,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Item {
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px > self.x && py > self.y && px < self.x + self.w && py < self.y + self.h
    }

    /// For folders: is the point on the frame / title bar (not the content area)?
    pub fn on_frame(&self, px: i32, py: i32) -> bool {
        px < self.x + 3 || py < self.y + 12 || px > self.x + self.w - 3 || py > self.y + self.h - 3
    }
}

struct Ctx<'a> {
    out: &'a mut Vec<Item>,
    p: Params,
    hmin: i32,
    vmin: i32,
}

/// Lay out `folder` (located at `path` in the tree) into the rectangle.
pub fn build(folder: &Folder, path: Vec<usize>, w: i32, h: i32, p: Params) -> Vec<Item> {
    let mut out = Vec::new();
    let (hmin, vmin) = MIN_SIZES[(p.density.clamp(-3, 3) + 3) as usize];
    let mut cx = Ctx { out: &mut out, p, hmin, vmin };

    let depth = path.len() as i32;
    layout_folder(&mut cx, folder, Rc::from(path), 0, 0, w - 1, h - 1, depth);
    out
}

#[allow(clippy::too_many_arguments)]
fn layout_folder(cx: &mut Ctx, folder: &Folder, path: Rc<[usize]>, x: i32, y: i32, w: i32, h: i32, depth: i32) {
    let indices: Vec<usize> = (0..folder.entries.len()).collect();
    split(cx, folder, &path, &indices, x, y, w, h, depth);
}

#[allow(clippy::too_many_arguments)]
fn split(cx: &mut Ctx, folder: &Folder, path: &Rc<[usize]>, indices: &[usize], x: i32, y: i32, w: i32, h: i32, depth: i32) {
    let mut l1 = Vec::new();
    let mut l2 = Vec::new();
    let (mut s1, mut s2) = (0u128, 0u128);
    for &i in indices {
        let e = &folder.entries[i];
        let size = if matches!(e.kind, Kind::Free) && !cx.p.show_free { 0 } else { e.size };
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
        return;
    }

    let (wbias, hbias) = match cx.p.bias {
        b if b > 0 => (b + 8, 8),
        b if b < 0 => (8, -b + 8),
        _ => (8, 8),
    };

    let (r1, r2) = if w * wbias / 8 > h * hbias / 8 {
        let sp = (w.max(0) as u128 * s1 / (s1 + s2)) as i32;
        ((x, y, sp, h), (x + sp, y, w - sp, h))
    } else {
        let sp = (h.max(0) as u128 * s1 / (s1 + s2)) as i32;
        ((x, y, w, sp), (x, y + sp, w, h - sp))
    };

    place(cx, folder, path, &l1, r1, depth);
    place(cx, folder, path, &l2, r2, depth);
}

fn place(cx: &mut Ctx, folder: &Folder, path: &Rc<[usize]>, list: &[usize], r: (i32, i32, i32, i32), depth: i32) {
    let (x, y, w, h) = r;
    let labeled = w > cx.hmin && h > cx.vmin;
    let visible = w > MIN_BOX && h > MIN_BOX;
    if list.len() > 1 && visible {
        split(cx, folder, path, list, x, y, w, h, depth);
    } else if list.len() == 1 && visible {
        let i = list[0];
        let e = &folder.entries[i];
        let is_free = matches!(e.kind, Kind::Free);
        cx.out.push(Item {
            folder: path.clone(),
            index: Some(i),
            depth: if is_free { -1 } else { depth },
            is_folder: e.child().is_some(),
            is_free,
            labeled,
            x,
            y,
            w,
            h,
        });
        if let (Some(child), true) = (e.child(), labeled) {
            let mut cp = path.to_vec();
            cp.push(i);
            layout_folder(cx, child, Rc::from(cp), x + 3, y + 12, w - 6, h - 15, depth + 1);
        }
    } else if !list.is_empty() {
        // A few pixels across: merge into one anonymous block.
        cx.out.push(Item {
            folder: path.clone(),
            index: None,
            depth,
            is_folder: false,
            is_free: false,
            labeled: false,
            x,
            y,
            w,
            h,
        });
    }
}

/// Item under the point, ignoring folder content areas, anonymous blocks and free space.
pub fn hit_test(items: &[Item], px: i32, py: i32) -> Option<usize> {
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
        Entry { name: name.into(), size, actual: size, mtime: 0, kind: Kind::File }
    }

    #[test]
    fn areas_roughly_proportional() {
        let f = Folder {
            entries: vec![file("a", 600), file("b", 300), file("c", 100)],
            total: 1000,
        };
        let items = build(&f, vec![], 1001, 501, Params::default());
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
        assert_eq!(hit_test(&items, it.x + it.w / 2, it.y + it.h / 2), Some(a));
    }

    #[test]
    fn small_boxes_are_items_without_labels() {
        let f = Folder {
            entries: vec![file("big", 9000), file("s1", 300), file("s2", 200)],
            total: 9500,
        };
        let items = build(&f, vec![], 400, 300, Params::default());
        let s1 = items.iter().position(|i| i.index == Some(1)).expect("small file laid out");
        assert!(!items[s1].labeled);
        let it = &items[s1];
        assert_eq!(hit_test(&items, it.x + it.w / 2, it.y + it.h / 2), Some(s1));
    }
}
