//! Shift+drag rectangle selection.

use super::Selection;
use crate::core::layout::Item;
use crate::core::model::EntryRef;
use std::collections::HashSet;

/// A rectangle selection in progress (view coordinates).
pub struct Marquee {
    pub start: (f32, f32),
    pub end: (f32, f32),
    /// Selection to add to (Ctrl+Shift+drag), empty for a fresh selection.
    pub base: Selection,
}

impl Marquee {
    pub fn new(start: (f32, f32), base: Selection) -> Self {
        Self {
            start,
            end: start,
            base,
        }
    }

    /// The selection with the rectangle dragged to `end`: the base plus every entry inside it.
    pub fn selection(&self, items: &[Item], end: (f32, f32)) -> Selection {
        let mut sel = self.base.clone();
        for r in hits(items, self.start, end) {
            sel.add(r);
        }
        sel
    }
}

/// Entries whose box lies fully inside the rectangle `a`..`b`, skipping those inside a folder
/// that is itself picked.
pub fn hits(items: &[Item], a: (f32, f32), b: (f32, f32)) -> Vec<EntryRef> {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let mut picked: HashSet<Vec<usize>> = HashSet::new();
    let mut out = Vec::new();
    // Parents come before children in `items`.
    for it in items {
        let Some(i) = it.index else { continue };
        if it.is_free || it.x < x0 || it.y < y0 || it.x + it.w > x1 || it.y + it.h > y1 {
            continue;
        }
        if (1..=it.folder.len()).any(|k| picked.contains(&it.folder[..k])) {
            continue;
        }
        let r = EntryRef::new(it.folder.clone(), i);
        picked.insert(r.path());
        out.push(r);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn item(folder: &[usize], index: usize, x: f32, y: f32, w: f32, h: f32) -> Item {
        Item {
            folder: Rc::from(folder),
            index: Some(index),
            depth: folder.len() as i32,
            is_folder: false,
            is_free: false,
            labeled: true,
            x,
            y,
            w,
            h,
        }
    }

    #[test]
    fn picks_boxes_fully_inside_and_skips_children_of_picked() {
        let items = [
            item(&[], 0, 0.0, 0.0, 50.0, 50.0),
            item(&[0], 0, 5.0, 15.0, 20.0, 20.0),
            item(&[], 1, 50.0, 0.0, 50.0, 50.0),
        ];
        assert_eq!(
            hits(&items, (-1.0, -1.0), (60.0, 60.0)),
            vec![EntryRef::new(Rc::from(&[][..]), 0)]
        );
        assert_eq!(
            hits(&items, (1.0, 1.0), (60.0, 60.0)),
            vec![EntryRef::new(Rc::from(&[0][..]), 0)]
        );
        let m = Marquee::new((40.0, -1.0), Selection::default());
        assert_eq!(m.selection(&items, (101.0, 51.0)).len(), 1);
    }
}
