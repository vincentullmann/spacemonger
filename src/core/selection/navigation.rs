//! Arrow-key selection moves.

use super::Selection;
use crate::core::geometry::Rect;
use crate::core::layout::{neighbour, Dir};
use crate::core::model::EntryRef;
use std::rc::Rc;

/// An arrow-key move.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Nav {
    /// Alt+Up.
    Parent,
    /// Alt+Down: first (top-left) child.
    FirstChild,
    Sibling(Dir),
    /// Ctrl / Shift + arrow: add the primary's neighbour to the selection.
    Extend(Dir),
}

impl Selection {
    /// Apply an arrow-key move. `boxes(folder)` gives the boxes of a folder's entries as drawn
    /// now. Every selected entry moves (entries with nowhere to go stay), except for
    /// `Nav::Extend`, which grows the selection from the primary.
    pub fn navigate(&mut self, nav: Nav, boxes: impl Fn(&[usize]) -> Vec<(usize, Rect)>) {
        let next = |r: &EntryRef, d: Dir| {
            let b = boxes(&r.folder);
            b.iter()
                .find(|x| x.0 == r.index)
                .and_then(|&(_, from)| neighbour(&b, from, d))
                .map(|n| EntryRef::new(r.folder.clone(), n))
        };
        if let Nav::Extend(d) = nav {
            if let Some(target) = self.primary().and_then(|p| next(p, d)) {
                self.extend_to(target);
            }
            return;
        }
        let mut out = Selection::default();
        for r in self.iter() {
            let moved = match nav {
                Nav::Parent => r
                    .folder
                    .split_last()
                    .map(|(&last, up)| EntryRef::new(Rc::from(up), last)),
                Nav::FirstChild => {
                    let p: Rc<[usize]> = r.path().into();
                    boxes(&p).first().map(|&(c, _)| EntryRef::new(p, c))
                }
                Nav::Sibling(d) => next(r, d),
                Nav::Extend(_) => None,
            };
            out.add(moved.unwrap_or_else(|| r.clone()));
        }
        *self = out;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two rows of two boxes in every folder.
    fn boxes(_: &[usize]) -> Vec<(usize, Rect)> {
        vec![
            (0, Rect::new(0.0, 0.0, 10.0, 10.0)),
            (1, Rect::new(10.0, 0.0, 20.0, 10.0)),
            (2, Rect::new(0.0, 10.0, 10.0, 20.0)),
            (3, Rect::new(10.0, 10.0, 20.0, 20.0)),
        ]
    }

    fn r(folder: &[usize], index: usize) -> EntryRef {
        EntryRef::new(Rc::from(folder), index)
    }

    fn sel(items: &[EntryRef]) -> Selection {
        let mut s = Selection::default();
        s.set(items.iter().cloned());
        s
    }

    #[test]
    fn siblings_move_and_stay_at_edges() {
        let mut s = sel(&[r(&[5], 0), r(&[5], 1)]);
        s.navigate(Nav::Sibling(Dir::Down), boxes);
        assert_eq!(s, sel(&[r(&[5], 2), r(&[5], 3)]));
        s.navigate(Nav::Sibling(Dir::Right), boxes);
        // 2 moves onto 3, which stays put: the two collapse into one.
        assert_eq!(s, sel(&[r(&[5], 3)]));
    }

    #[test]
    fn parent_and_first_child() {
        let mut s = sel(&[r(&[1, 2], 3)]);
        s.navigate(Nav::Parent, boxes);
        assert_eq!(s, sel(&[r(&[1], 2)]));
        s.navigate(Nav::FirstChild, boxes);
        assert_eq!(s, sel(&[r(&[1, 2], 0)]));
        let mut top = sel(&[r(&[], 0)]);
        top.navigate(Nav::Parent, boxes);
        assert_eq!(top, sel(&[r(&[], 0)]));
    }

    #[test]
    fn extend_grows_from_primary() {
        let mut s = sel(&[r(&[], 0)]);
        s.navigate(Nav::Extend(Dir::Right), boxes);
        s.navigate(Nav::Extend(Dir::Down), boxes);
        assert_eq!(s, sel(&[r(&[], 0), r(&[], 1), r(&[], 3)]));
        s.navigate(Nav::Extend(Dir::Up), boxes);
        assert_eq!(s, sel(&[r(&[], 0), r(&[], 1)]));
    }
}
