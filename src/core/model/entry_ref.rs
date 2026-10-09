//! A reference to one entry in the tree by index path.

use std::rc::Rc;

/// An entry, identified by its folder's index path and its index within that folder.
///
/// Indices shift when an earlier sibling is removed, so references only stay valid until the
/// tree changes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntryRef {
    pub folder: Rc<[usize]>,
    pub index: usize,
}

impl EntryRef {
    pub fn new(folder: Rc<[usize]>, index: usize) -> Self {
        Self { folder, index }
    }

    /// Full index path from the tree root (folder path + index).
    pub fn path(&self) -> Vec<usize> {
        let mut p = self.folder.to_vec();
        p.push(self.index);
        p
    }
}
