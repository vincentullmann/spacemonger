//! A scanned tree with lookup by index path, hide / unhide and removal.

use std::path::PathBuf;

use super::{Entry, EntryRef, Folder, Kind};

pub struct Tree {
    pub root: Folder,
    pub root_path: PathBuf,
    pub total_space: u64,
    pub free_space: u64,
    pub num_files: u64,
    pub num_folders: u64,
    /// Folders and entries the scan couldn't read.
    pub unreadable: u64,
    /// Number of entries currently hidden from the view by the user.
    pub hidden_count: usize,
    /// Entries named `.something` are left out (see [`Tree::set_dotfiles_hidden`]).
    pub dotfiles_hidden: bool,
}

impl Tree {
    pub fn folder_at(&self, path: &[usize]) -> Option<&Folder> {
        self.root.descendant(path)
    }

    pub fn entry_at(&self, folder: &[usize], index: usize) -> Option<&Entry> {
        self.folder_at(folder)?.entries.get(index)
    }

    pub fn entry(&self, r: &EntryRef) -> Option<&Entry> {
        self.entry_at(&r.folder, r.index)
    }

    /// Absolute filesystem path of a referenced entry.
    pub fn path_of(&self, r: &EntryRef) -> PathBuf {
        self.full_path(&r.folder, Some(r.index))
    }

    /// Absolute filesystem path of a folder (and optionally one of its entries).
    pub fn full_path(&self, folder: &[usize], index: Option<usize>) -> PathBuf {
        let mut p = self.root_path.clone();
        let mut f = &self.root;
        for &i in folder {
            // A stale path (the tree changed under it) stops at the last folder that exists.
            let Some(e) = f.entries.get(i) else { return p };
            p.push(&*e.name);
            match e.child() {
                Some(c) => f = c,
                None => return p,
            }
        }
        if let Some(i) = index {
            if let Some(e) = f.entries.get(i) {
                p.push(&*e.name);
            }
        }
        p
    }

    /// Hide an entry from the view: ancestors shrink by its size; `unhide_all` restores it.
    pub fn hide(&mut self, folder: &[usize], index: usize) {
        let Some(e) = self.entry_at(folder, index).filter(|e| !e.hidden) else {
            return;
        };
        // A dotfile left out of the sizes already has nothing left to subtract.
        let size = if self.is_left_out(e) { 0 } else { e.size };
        self.shrink_ancestors(folder, size);
        if let Some(f) = self.folder_at_mut(folder) {
            f.entries[index].hidden = true;
        }
        self.hidden_count += 1;
    }

    /// Bring back every entry the user hid and restore ancestor sizes.
    pub fn unhide_all(&mut self) {
        fn walk(f: &mut Folder) {
            for e in &mut f.entries {
                e.hidden = false;
                if let Kind::Dir(c) = &mut e.kind {
                    walk(c);
                }
            }
        }
        walk(&mut self.root);
        self.hidden_count = 0;
        self.recompute_sizes();
    }

    /// Leave out (or bring back) entries whose name starts with a dot.
    pub fn set_dotfiles_hidden(&mut self, on: bool) {
        if self.dotfiles_hidden != on {
            self.dotfiles_hidden = on;
            self.recompute_sizes();
        }
    }

    /// Whether an entry is left out of folder sizes and the layout.
    pub fn is_left_out(&self, e: &Entry) -> bool {
        e.hidden || (self.dotfiles_hidden && e.is_dotfile())
    }

    /// Recompute every folder's size from the entries that are still shown.
    fn recompute_sizes(&mut self) {
        fn walk(f: &mut Folder, dots: bool) -> u64 {
            let mut total = 0;
            for e in &mut f.entries {
                if let Kind::Dir(c) = &mut e.kind {
                    e.size = walk(c, dots);
                    e.actual = e.size;
                }
                if !(e.hidden || (dots && e.is_dotfile())) {
                    total += e.size;
                }
            }
            f.total = total;
            total
        }
        walk(&mut self.root, self.dotfiles_hidden);
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

    /// Remove an entry after it has been deleted on disk, updating ancestor sizes and the
    /// file / folder counts.
    pub fn remove(&mut self, folder: &[usize], index: usize) {
        let Some(e) = self.entry_at(folder, index) else {
            return;
        };
        let size = if self.is_left_out(e) { 0 } else { e.size };
        let (files, folders) = e.counts();
        self.shrink_ancestors(folder, size);
        if let Some(f) = self.folder_at_mut(folder) {
            f.entries.remove(index);
        }
        self.num_files = self.num_files.saturating_sub(files);
        self.num_folders = self.num_folders.saturating_sub(folders);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::testing::file;

    /// A tree over `root` with the counts the tests start from.
    fn tree(root: Folder, root_path: &str) -> Tree {
        Tree {
            root,
            root_path: PathBuf::from(root_path),
            total_space: 0,
            free_space: 0,
            num_files: 3,
            num_folders: 1,
            unreadable: 0,
            hidden_count: 0,
            dotfiles_hidden: false,
        }
    }

    #[test]
    fn hide_and_unhide_restore_sizes() {
        let sub = Folder {
            entries: vec![file("f", 10), file("g", 5)],
            total: 15,
        };
        let root = Folder {
            entries: vec![Entry::new("d", 15, Kind::Dir(Box::new(sub))), file("x", 7)],
            total: 22,
        };
        let mut t = tree(root, "/");
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

    #[test]
    fn dotfiles_leave_and_come_back() {
        let sub = Folder {
            entries: vec![file(".git", 10), file("g", 5)],
            total: 15,
        };
        let root = Folder {
            entries: vec![Entry::new("d", 15, Kind::Dir(Box::new(sub))), file(".x", 7)],
            total: 22,
        };
        let mut t = tree(root, "/");
        t.set_dotfiles_hidden(true);
        assert_eq!((t.root.total, t.root.entries[0].size), (5, 5));
        t.hide(&[0], 1); // d/g
        assert_eq!(t.root.total, 0);
        t.unhide_all();
        assert_eq!(t.root.total, 5);
        t.set_dotfiles_hidden(false);
        assert_eq!((t.root.total, t.root.entries[0].size), (22, 15));
    }

    #[test]
    fn remove_updates_counts_and_survives_stale_paths() {
        let sub = Folder {
            entries: vec![file("f", 10), file("g", 5)],
            total: 15,
        };
        let root = Folder {
            entries: vec![Entry::new("d", 15, Kind::Dir(Box::new(sub))), file("x", 7)],
            total: 22,
        };
        let mut t = tree(root, "/r");
        t.remove(&[], 0); // d, with its two files
        assert_eq!((t.num_files, t.num_folders, t.root.total), (1, 0, 7));
        // A path into the removed folder no longer panics.
        assert_eq!(t.full_path(&[5, 1], Some(0)), PathBuf::from("/r"));
    }

    #[test]
    fn hiding_a_left_out_dotfile_subtracts_nothing() {
        let mut t = tree(
            Folder {
                entries: vec![file(".x", 7), file("y", 3)],
                total: 10,
            },
            "/",
        );
        t.set_dotfiles_hidden(true);
        t.hide(&[], 0);
        assert_eq!(t.root.total, 3);
        t.unhide_all();
        assert_eq!(t.root.total, 3);
    }
}
