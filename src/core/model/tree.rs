//! A scanned tree with lookup by index path, hide / unhide and removal.

use super::{Entry, Folder, Kind};
use std::path::PathBuf;

pub struct Tree {
    pub root: Folder,
    pub root_path: PathBuf,
    pub total_space: u64,
    pub free_space: u64,
    pub num_files: u64,
    pub num_folders: u64,
    /// Number of entries currently hidden from the view.
    pub hidden_count: usize,
}

impl Tree {
    pub fn folder_at(&self, path: &[usize]) -> Option<&Folder> {
        self.root.descendant(path)
    }

    pub fn entry_at(&self, folder: &[usize], index: usize) -> Option<&Entry> {
        self.folder_at(folder)?.entries.get(index)
    }

    /// Absolute filesystem path of a folder (and optionally one of its entries).
    pub fn full_path(&self, folder: &[usize], index: Option<usize>) -> PathBuf {
        let mut p = self.root_path.clone();
        let mut f = &self.root;
        for &i in folder {
            let e = &f.entries[i];
            p.push(&e.name);
            match e.child() {
                Some(c) => f = c,
                None => return p,
            }
        }
        if let Some(i) = index {
            if let Some(e) = f.entries.get(i) {
                p.push(&e.name);
            }
        }
        p
    }

    /// Hide an entry from the view: ancestors shrink by its size; `unhide_all` restores it.
    pub fn hide(&mut self, folder: &[usize], index: usize) {
        let Some(size) = self.entry_at(folder, index).filter(|e| !e.hidden).map(|e| e.size) else {
            return;
        };
        self.shrink_ancestors(folder, size);
        if let Some(f) = self.folder_at_mut(folder) {
            f.entries[index].hidden = true;
        }
        self.hidden_count += 1;
    }

    /// Bring back every hidden entry and restore ancestor sizes.
    pub fn unhide_all(&mut self) {
        fn walk(f: &mut Folder) -> u64 {
            let mut restored = 0;
            for e in &mut f.entries {
                let inner = match &mut e.kind {
                    Kind::Dir(c) => walk(c),
                    _ => 0,
                };
                e.size += inner;
                e.actual += inner;
                if e.hidden {
                    e.hidden = false;
                    restored += e.size;
                } else {
                    restored += inner;
                }
            }
            f.total += restored;
            restored
        }
        walk(&mut self.root);
        self.hidden_count = 0;
    }

    fn folder_at_mut(&mut self, path: &[usize]) -> Option<&mut Folder> {
        let mut f = &mut self.root;
        for &i in path {
            f = match &mut f.entries.get_mut(i)?.kind {
                Kind::Dir(c) => c,
                _ => return None,
            };
        }
        Some(f)
    }

    /// Subtract `size` from the folder at `folder` and all its ancestors.
    fn shrink_ancestors(&mut self, folder: &[usize], size: u64) {
        let mut f = &mut self.root;
        for &i in folder {
            f.total = f.total.saturating_sub(size);
            let e = &mut f.entries[i];
            e.size = e.size.saturating_sub(size);
            e.actual = e.actual.saturating_sub(size);
            f = match &mut e.kind {
                Kind::Dir(c) => c,
                _ => return,
            };
        }
        f.total = f.total.saturating_sub(size);
    }

    /// Remove an entry after it has been deleted on disk, updating ancestor sizes.
    pub fn remove(&mut self, folder: &[usize], index: usize) {
        let Some(size) = self.entry_at(folder, index).map(|e| e.size) else {
            return;
        };
        let mut f = &mut self.root;
        for &i in folder {
            f.total = f.total.saturating_sub(size);
            let e = &mut f.entries[i];
            e.size = e.size.saturating_sub(size);
            e.actual = e.actual.saturating_sub(size);
            f = match &mut e.kind {
                Kind::Dir(c) => c,
                _ => return,
            };
        }
        f.total = f.total.saturating_sub(size);
        if index < f.entries.len() {
            f.entries.remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: u64) -> Entry {
        Entry { name: name.into(), size, actual: size, mtime: 0, kind: Kind::File, hidden: false }
    }

    #[test]
    fn hide_and_unhide_restore_sizes() {
        let sub = Folder { entries: vec![file("f", 10), file("g", 5)], total: 15 };
        let root = Folder {
            entries: vec![
                Entry { name: "d".into(), size: 15, actual: 15, mtime: 0, kind: Kind::Dir(Box::new(sub)), hidden: false },
                file("x", 7),
            ],
            total: 22,
        };
        let mut t = Tree {
            root,
            root_path: PathBuf::from("/"),
            total_space: 0,
            free_space: 0,
            num_files: 3,
            num_folders: 1,
            hidden_count: 0,
        };
        t.hide(&[0], 0); // d/f
        assert_eq!((t.root.total, t.root.entries[0].size), (12, 5));
        t.hide(&[], 0); // d
        assert_eq!(t.root.total, 7);
        assert_eq!(t.hidden_count, 2);
        t.unhide_all();
        assert_eq!((t.root.total, t.root.entries[0].size), (22, 15));
        assert_eq!(t.folder_at(&[0]).unwrap().total, 15);
        assert_eq!(t.hidden_count, 0);
        assert!(!t.root.entries[0].hidden);
    }
}
