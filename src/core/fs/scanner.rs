//! Parallel scanner: stays on one filesystem and counts hard links once.

use super::metadata::meta;
use super::Drive;
use crate::core::model::{Entry, Folder, Kind, Tree};
use rayon::prelude::*;
use dashmap::DashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;


/// Shared state between the scanner threads and the UI.
pub struct ScanControl {
    pub cancelled: AtomicBool,
    pub files: AtomicU64,
    pub folders: AtomicU64,
    pub bytes: AtomicU64,
    pub current: Mutex<String>,
    /// (device, inode) of hard-linked files and folders already counted.
    seen: DashSet<(u64, u64)>,
}

impl Default for ScanControl {
    fn default() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            files: AtomicU64::new(0),
            folders: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            current: Mutex::new(String::new()),
            seen: DashSet::new(),
        }
    }
}

impl ScanControl {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    /// Returns false if this (device, inode) was already counted (hard links, bind mounts).
    fn first_sighting(&self, dev: u64, ino: u64) -> bool {
        self.seen.insert((dev, ino))
    }
}

fn scan_dir(path: &Path, dev: u64, ctl: &ScanControl) -> Folder {
    let mut folder = Folder::default();
    if ctl.is_cancelled() {
        return folder;
    }
    if let Ok(mut cur) = ctl.current.try_lock() {
        *cur = path.display().to_string();
    }

    let Ok(rd) = fs::read_dir(path) else {
        return folder;
    };
    let mut names: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    names.sort();

    let mut subdirs = Vec::new();
    for name in names {
        if ctl.is_cancelled() {
            break;
        }
        let child = path.join(&name);
        let Some(m) = meta(&child) else { continue };
        let name = name.to_string_lossy().to_string();
        if m.is_dir {
            if m.dev != dev || !ctl.first_sighting(m.dev, m.ino) {
                continue; // other filesystem, or already seen via a bind mount
            }
            subdirs.push((name, child, m.mtime));
        } else if m.is_file {
            if m.nlink > 1 && !ctl.first_sighting(m.dev, m.ino) {
                continue; // hard link already counted
            }
            folder.entries.push(Entry {
                name,
                size: m.alloc,
                actual: m.len,
                mtime: m.mtime,
                kind: Kind::File,
                hidden: false,
            });
            ctl.files.fetch_add(1, Ordering::Relaxed);
            ctl.bytes.fetch_add(m.alloc, Ordering::Relaxed);
        }
    }

    let children: Vec<_> = subdirs
        .into_par_iter()
        .map(|(name, p, mtime)| {
            let sub = scan_dir(&p, dev, ctl);
            ctl.folders.fetch_add(1, Ordering::Relaxed);
            (name, sub, mtime)
        })
        .collect();
    for (name, sub, mtime) in children {
        let size = sub.total;
        folder.entries.push(Entry {
            name,
            size,
            actual: size,
            mtime,
            kind: Kind::Dir(Box::new(sub)),
            hidden: false,
        });
    }

    folder.finalize();
    folder
}

/// Scan a drive. Returns `None` if cancelled.
pub fn scan(drive: &Drive, ctl: &ScanControl) -> Option<Tree> {
    let root_meta = meta(&drive.root)?;
    let pool = rayon::ThreadPoolBuilder::new()
        .stack_size(32 * 1024 * 1024)
        .build()
        .ok()?;
    let mut root = pool.install(|| scan_dir(&drive.root, root_meta.dev, ctl));
    if ctl.is_cancelled() {
        return None;
    }
    root.entries.push(Entry {
        name: String::new(),
        size: drive.free,
        actual: drive.free,
        mtime: 0,
        kind: Kind::Free,
        hidden: false,
    });
    root.finalize();
    Some(Tree {
        root,
        root_path: drive.root.clone(),
        total_space: drive.total,
        free_space: drive.free,
        num_files: ctl.files.load(Ordering::Relaxed),
        num_folders: ctl.folders.load(Ordering::Relaxed),
        hidden_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::fs::drive_for_path;

    #[test]
    fn scans_and_removes() {
        let dir = std::env::temp_dir().join(format!("sm_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("a/b")).unwrap();
        fs::write(dir.join("a/b/big"), vec![1u8; 100_000]).unwrap();
        fs::write(dir.join("a/small"), vec![1u8; 10]).unwrap();
        fs::write(dir.join("top"), vec![1u8; 50_000]).unwrap();

        let drive = drive_for_path(&dir).unwrap();
        let ctl = ScanControl::default();
        let mut tree = scan(&drive, &ctl).unwrap();
        assert_eq!(tree.num_files, 3);
        assert_eq!(tree.num_folders, 2);

        // root: [free?, a, top] sorted by size; find "a"
        let ai = tree.root.entries.iter().position(|e| e.name == "a").unwrap();
        let a_size = tree.root.entries[ai].size;
        assert!(a_size >= 100_000);
        assert_eq!(
            tree.full_path(&[ai], Some(0)),
            dir.canonicalize().unwrap().join("a").join("b")
        );

        // remove a/b
        let b_size = tree.entry_at(&[ai], 0).unwrap().size;
        let root_total = tree.root.total;
        tree.remove(&[ai], 0);
        assert_eq!(tree.root.total, root_total - b_size);
        assert_eq!(tree.root.entries[ai].size, a_size - b_size);
        assert_eq!(tree.folder_at(&[ai]).unwrap().entries.len(), 1);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn hard_links_count_once() {
        let dir = std::env::temp_dir().join(format!("sm_test_links_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("f"), vec![1u8; 10_000]).unwrap();
        fs::hard_link(dir.join("f"), dir.join("sub/g")).unwrap();

        let tree = scan(&drive_for_path(&dir).unwrap(), &ScanControl::default()).unwrap();
        assert_eq!(tree.num_files, 1);

        fs::remove_dir_all(&dir).unwrap();
    }
}
