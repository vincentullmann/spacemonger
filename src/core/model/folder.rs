//! A folder's entries, sorted largest first.

use super::Entry;

#[derive(Default)]
pub struct Folder {
    /// Sorted by `size`, descending.
    pub entries: Vec<Entry>,
    pub total: u64,
}

impl Folder {
    /// The folder at index path `path` below this one (`self` for an empty path).
    pub fn descendant(&self, path: &[usize]) -> Option<&Folder> {
        let mut f = self;
        for &i in path {
            f = f.entries.get(i)?.child()?;
        }
        Some(f)
    }

    /// Sort entries largest first and recompute the total.
    pub fn finalize(&mut self) {
        // Stable sort keeps name order for equal sizes.
        self.entries.sort_by_key(|e| std::cmp::Reverse(e.size));
        self.total = self.entries.iter().map(|e| e.size).sum();
    }
}
