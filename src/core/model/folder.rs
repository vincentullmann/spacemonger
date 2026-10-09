//! A folder's entries, sorted largest first.

use super::Entry;

#[derive(Default)]
pub struct Folder {
    /// Sorted by `size`, descending.
    pub entries: Vec<Entry>,
    pub total: u64,
}

impl Folder {
    /// Sort entries largest first and recompute the total.
    pub fn finalize(&mut self) {
        // Stable sort keeps name order for equal sizes.
        self.entries.sort_by_key(|e| std::cmp::Reverse(e.size));
        self.total = self.entries.iter().map(|e| e.size).sum();
    }
}
