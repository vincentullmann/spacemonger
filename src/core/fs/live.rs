//! The tree as it's being scanned: scanner threads fill it in while the UI takes snapshots.

use crate::core::model::{Entry, Folder, Kind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// A folder whose contents are being scanned.
pub(super) struct LiveDir {
    name: String,
    mtime: i64,
    created: i64,
    /// Bytes found so far in this folder and below.
    bytes: AtomicU64,
    /// Set once, when the folder has been listed.
    contents: OnceLock<Contents>,
}

struct Contents {
    /// Largest first.
    files: Vec<Entry>,
    dirs: Vec<LiveDir>,
}

/// A folder plus its ancestors, so byte counts can be added all the way up.
pub(super) struct Chain<'a> {
    pub dir: &'a LiveDir,
    pub parent: Option<&'a Chain<'a>>,
}

impl Chain<'_> {
    pub fn add_bytes(&self, n: u64) {
        let mut c = Some(self);
        while let Some(link) = c {
            link.dir.bytes.fetch_add(n, Ordering::Relaxed);
            c = link.parent;
        }
    }
}

impl LiveDir {
    pub fn new(name: String, mtime: i64, created: i64) -> Self {
        Self { name, mtime, created, bytes: AtomicU64::new(0), contents: OnceLock::new() }
    }

    pub fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    /// Record the folder's listing (files sorted largest first) and return its subfolders.
    pub fn set_contents(&self, files: Vec<Entry>, dirs: Vec<LiveDir>) -> &[LiveDir] {
        let _ = self.contents.set(Contents { files, dirs });
        self.contents.get().map_or(&[], |c| &c.dirs)
    }

    /// Copy of what's been found so far, leaving out entries under `min` bytes.
    pub fn snapshot(&self, min: u64) -> Folder {
        let mut f = Folder::default();
        if let Some(c) = self.contents.get() {
            for e in c.files.iter().take_while(|e| e.size >= min) {
                f.entries.push(Entry { name: e.name.clone(), kind: Kind::File, ..*e });
            }
            for d in &c.dirs {
                let size = d.bytes();
                if size >= min && size > 0 {
                    // Sized by everything found so far, including what the snapshot leaves out.
                    f.entries.push(dir_entry(d.name.clone(), (d.mtime, d.created), size, d.snapshot(min)));
                }
            }
        }
        f.finalize();
        f
    }

    /// The finished folder, once every scanner thread is done with it.
    pub fn into_folder(self) -> Folder {
        let mut f = Folder::default();
        if let Some(c) = self.contents.into_inner() {
            f.entries = c.files;
            for d in c.dirs {
                let (name, times) = (d.name.clone(), (d.mtime, d.created));
                let sub = d.into_folder();
                f.entries.push(dir_entry(name, times, sub.total, sub));
            }
        }
        f.finalize();
        f
    }
}

fn dir_entry(name: String, (mtime, created): (i64, i64), size: u64, folder: Folder) -> Entry {
    Entry { name, size, actual: size, mtime, created, kind: Kind::Dir(Box::new(folder)), hidden: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: u64) -> Entry {
        Entry { name: name.into(), size, actual: size, mtime: 0, created: 0, kind: Kind::File, hidden: false }
    }

    #[test]
    fn snapshot_while_filling_in() {
        let root = LiveDir::new(String::new(), 0, 0);
        let root_chain = Chain { dir: &root, parent: None };
        let dirs = root.set_contents(vec![file("big", 1000), file("tiny", 1)], vec![LiveDir::new("d".into(), 0, 0)]);
        root_chain.add_bytes(1001);

        // "d" hasn't been listed yet: it has no bytes, so it's left out.
        let s = root.snapshot(0);
        assert_eq!(s.entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["big", "tiny"]);

        let d_chain = Chain { dir: &dirs[0], parent: Some(&root_chain) };
        dirs[0].set_contents(vec![file("f", 5000)], Vec::new());
        d_chain.add_bytes(5000);
        assert_eq!(root.bytes(), 6001);

        // Small entries are dropped from snapshots, but still counted in their folder's size.
        let s = root.snapshot(10);
        assert_eq!(s.entries.iter().map(|e| (e.name.as_str(), e.size)).collect::<Vec<_>>(), [("d", 5000), ("big", 1000)]);
        assert_eq!(s.entries[0].child().unwrap().entries.len(), 1);

        let f = root.into_folder();
        assert_eq!(f.total, 6001);
        assert_eq!(f.entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["d", "big", "tiny"]);
    }
}
