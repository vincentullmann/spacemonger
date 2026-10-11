//! Small trees for tests.

use super::{Entry, Folder, Kind};

/// A file of `size` bytes.
pub fn file(name: &str, size: u64) -> Entry {
    Entry::new(name, size, Kind::File)
}

/// A folder holding `entries`, sorted and sized like a scanned one.
pub fn dir(name: &str, entries: Vec<Entry>) -> Entry {
    let f = folder(entries);
    Entry::new(name, f.total, Kind::Dir(Box::new(f)))
}

/// A folder of `entries`, sorted largest first, with its total.
pub fn folder(entries: Vec<Entry>) -> Folder {
    let mut f = Folder { entries, total: 0 };
    f.finalize();
    f
}
