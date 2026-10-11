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
    /// Entry `index` of the folder at index path `folder`.
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

/// Where the entry at index path `path` ends up after the entry `index` in `folder` was
/// removed: later siblings (and everything below them) move up by one. `None` if `path` is
/// the removed entry or inside it.
pub fn path_after_remove(path: &[usize], folder: &[usize], index: usize) -> Option<Vec<usize>> {
    let depth = folder.len();
    if path.len() <= depth || path[..depth] != *folder {
        return Some(path.to_vec());
    }
    let mut out = path.to_vec();
    match path[depth].cmp(&index) {
        std::cmp::Ordering::Less => {}
        std::cmp::Ordering::Equal => return None,
        std::cmp::Ordering::Greater => out[depth] -= 1,
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_shift_after_remove() {
        // Outside the folder, or before the removed entry: unchanged.
        assert_eq!(path_after_remove(&[1, 2], &[0], 0), Some(vec![1, 2]));
        assert_eq!(path_after_remove(&[0, 1], &[0], 3), Some(vec![0, 1]));
        // The folder itself is untouched.
        assert_eq!(path_after_remove(&[0], &[0], 0), Some(vec![0]));
        // Later siblings and their contents move up.
        assert_eq!(path_after_remove(&[0, 4, 1], &[0], 2), Some(vec![0, 3, 1]));
        // The removed entry and anything inside it are gone.
        assert_eq!(path_after_remove(&[0, 2], &[0], 2), None);
        assert_eq!(path_after_remove(&[0, 2, 5], &[0], 2), None);
    }
}
