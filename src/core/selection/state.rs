//! The set of selected entries.

use crate::core::model::EntryRef;
use indexmap::IndexSet;
use std::collections::HashSet;

/// Selected entries in the order they were picked; the last one is the primary (used by
/// Zoom In, the window title and Shift+arrow extension). Equality ignores order.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Selection {
    items: IndexSet<EntryRef>,
}

impl Selection {
    pub fn primary(&self) -> Option<&EntryRef> {
        self.items.last()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &EntryRef> {
        self.items.iter()
    }

    pub fn contains(&self, r: &EntryRef) -> bool {
        self.items.contains(r)
    }

    /// Is `r` the one and only selected entry?
    pub fn is_only(&self, r: &EntryRef) -> bool {
        self.items.len() == 1 && self.items.contains(r)
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Replace the selection.
    pub fn set(&mut self, items: impl IntoIterator<Item = EntryRef>) {
        self.items = items.into_iter().collect();
    }

    /// Add an entry unless it's already in (Shift+click).
    pub fn add(&mut self, r: EntryRef) {
        self.items.insert(r);
    }

    /// Add an entry, or take it out if it's already in (Ctrl+click).
    pub fn toggle(&mut self, r: EntryRef) {
        if !self.items.shift_remove(&r) {
            self.items.insert(r);
        }
    }

    /// Make `target` the new primary, growing the selection; stepping back onto the entry
    /// added just before undoes the last step, so it shrinks again like in a list.
    pub fn extend_to(&mut self, target: EntryRef) {
        let len = self.items.len();
        if len >= 2 && self.items[len - 2] == target {
            self.items.pop();
        } else {
            self.items.shift_remove(&target);
            self.items.insert(target);
        }
    }

    /// The selection without duplicates or entries inside another selected folder (they go
    /// with it), sorted by path, last first: removing one then never shifts the indices of
    /// those still to come.
    pub fn roots(&self) -> Vec<EntryRef> {
        let all: HashSet<Vec<usize>> = self.items.iter().map(EntryRef::path).collect();
        let mut roots: Vec<EntryRef> = self
            .items
            .iter()
            .filter(|r| !(1..=r.folder.len()).any(|k| all.contains(&r.folder[..k])))
            .cloned()
            .collect();
        roots.sort_by_key(|r| std::cmp::Reverse(r.path()));
        roots.dedup();
        roots
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn r(folder: &[usize], index: usize) -> EntryRef {
        EntryRef::new(Rc::from(folder), index)
    }

    #[test]
    fn toggle_add_and_primary() {
        let mut s = Selection::default();
        s.add(r(&[], 0));
        s.add(r(&[], 0));
        assert_eq!(s.len(), 1);
        assert!(s.is_only(&r(&[], 0)));
        s.toggle(r(&[], 1));
        assert_eq!(s.primary(), Some(&r(&[], 1)));
        s.toggle(r(&[], 1));
        assert_eq!(s.primary(), Some(&r(&[], 0)));
    }

    #[test]
    fn extend_steps_back() {
        let mut s = Selection::default();
        s.set([r(&[], 0)]);
        s.extend_to(r(&[], 1));
        s.extend_to(r(&[], 2));
        assert_eq!(s.len(), 3);
        // Back onto the previous one: undo the last step.
        s.extend_to(r(&[], 1));
        assert_eq!(
            s.iter().cloned().collect::<Vec<_>>(),
            vec![r(&[], 0), r(&[], 1)]
        );
    }

    #[test]
    fn roots_drop_nested_and_sort_last_first() {
        let mut s = Selection::default();
        s.set([r(&[0], 2), r(&[], 0), r(&[], 3), r(&[1], 0)]);
        // [0, 2] is inside [0], which is selected.
        assert_eq!(s.roots(), vec![r(&[], 3), r(&[1], 0), r(&[], 0)]);
    }
}
