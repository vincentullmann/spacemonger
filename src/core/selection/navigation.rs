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
                Nav::Parent => r.folder.split_last().map(|(&last, up)| EntryRef::new(Rc::from(up), last)),
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
