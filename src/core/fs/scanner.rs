//! Parallel scanner: by default stays on one filesystem and counts hard links once.

use super::live::{Chain, LiveDir};
use super::metadata::meta;
use super::Drive;
use crate::core::model::{Entry, Folder, Kind, Tree};
use dashmap::DashSet;
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// What the scanner follows and counts.
#[derive(Clone, Copy, Debug)]
pub struct ScanOptions {
    /// Don't descend into other mounted filesystems.
    pub one_filesystem: bool,
    /// Count a file with several hard links once.
    pub hardlinks_once: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            one_filesystem: true,
            hardlinks_once: true,
        }
    }
}

/// Shared state between the scanner threads and the UI.
pub struct ScanControl {
    pub cancelled: AtomicBool,
    /// Scanner threads wait while this is set (at the same points they check for a cancel).
    pub paused: AtomicBool,
    pub files: AtomicU64,
    pub folders: AtomicU64,
    pub bytes: AtomicU64,
    pub current: Mutex<String>,
    /// (device, inode) of hard-linked files and folders already counted.
    seen: DashSet<(u64, u64)>,
    /// The tree so far, while the scan runs.
    live: Mutex<Option<Arc<LiveDir>>>,
    pub opts: ScanOptions,
}

impl Default for ScanControl {
    fn default() -> Self {
        Self::new(ScanOptions::default())
    }
}

impl ScanControl {
    pub fn new(opts: ScanOptions) -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            files: AtomicU64::new(0),
            folders: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            current: Mutex::new(String::new()),
            seen: DashSet::new(),
            live: Mutex::new(None),
            opts,
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
    /// Whether to stop. While paused, waits here until resumed or cancelled.
    fn is_cancelled(&self) -> bool {
        while self.is_paused() && !self.cancelled.load(Ordering::Relaxed) {
            std::thread::sleep(PAUSE_POLL);
        }
        self.cancelled.load(Ordering::Relaxed)
    }
    /// Returns false if this (device, inode) was already counted (hard links, bind mounts).
    /// Entries without an id are always counted.
    fn first_sighting(&self, id: Option<(u64, u64)>) -> bool {
        id.is_none_or(|id| self.seen.insert(id))
    }
    fn live(&self) -> std::sync::MutexGuard<'_, Option<Arc<LiveDir>>> {
        self.live.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// What the scan of `drive` has found so far, for drawing while it runs. Entries too small
    /// to show up are left out. `None` before the scan starts and after it ends.
    pub fn snapshot(&self, drive: &Drive) -> Option<Tree> {
        let live = self.live();
        let root = live.as_ref()?;
        let min = root.bytes() / LIVE_DETAIL;
        Some(make_tree(drive, root.snapshot(min), self))
    }
}

/// How often a paused scanner thread looks whether it may go on.
const PAUSE_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// A live snapshot keeps entries of at least this fraction of the bytes found so far.
const LIVE_DETAIL: u64 = 200_000;

/// Scan the folder at `path` into `chain.dir`.
fn scan_dir(path: &Path, dev: u64, ctl: &ScanControl, chain: &Chain) {
    if ctl.is_cancelled() {
        return;
    }
    if let Ok(mut cur) = ctl.current.try_lock() {
        *cur = path.display().to_string();
    }

    let Ok(rd) = fs::read_dir(path) else {
        return;
    };
    let mut names: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    names.sort();

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut dir_paths = Vec::new();
    for name in names {
        if ctl.is_cancelled() {
            break;
        }
        let child = path.join(&name);
        let Some(m) = meta(&child) else { continue };
        let name = name.to_string_lossy().to_string();
        if m.is_dir {
            if (ctl.opts.one_filesystem && m.dev != dev) || !ctl.first_sighting(m.id) {
                continue; // other filesystem, or already seen via a bind mount
            }
            dirs.push(LiveDir::new(name, m.mtime, m.created));
            dir_paths.push(child);
        } else if m.is_file {
            if ctl.opts.hardlinks_once && m.nlink > 1 && !ctl.first_sighting(m.id) {
                continue; // hard link already counted
            }
            files.push(Entry {
                name,
                size: m.alloc,
                actual: m.len,
                mtime: m.mtime,
                created: m.created,
                kind: Kind::File,
                hidden: false,
            });
        }
    }

    // Stable sort keeps name order for equal sizes.
    files.sort_by_key(|e| std::cmp::Reverse(e.size));
    let file_bytes: u64 = files.iter().map(|e| e.size).sum();
    ctl.files.fetch_add(files.len() as u64, Ordering::Relaxed);
    ctl.bytes.fetch_add(file_bytes, Ordering::Relaxed);
    let dirs = chain.dir.set_contents(files, dirs);
    chain.add_bytes(file_bytes);

    dirs.par_iter().zip(dir_paths).for_each(|(dir, p)| {
        scan_dir(
            &p,
            dev,
            ctl,
            &Chain {
                dir,
                parent: Some(chain),
            },
        );
        ctl.folders.fetch_add(1, Ordering::Relaxed);
    });
}

/// A tree from a scanned (or partly scanned) root folder, with the free space block added.
fn make_tree(drive: &Drive, mut root: Folder, ctl: &ScanControl) -> Tree {
    root.entries.push(Entry {
        name: String::new(),
        size: drive.free,
        actual: drive.free,
        mtime: 0,
        created: 0,
        kind: Kind::Free,
        hidden: false,
    });
    root.finalize();
    Tree {
        root,
        root_path: drive.root.clone(),
        total_space: drive.total,
        free_space: drive.free,
        num_files: ctl.files.load(Ordering::Relaxed),
        num_folders: ctl.folders.load(Ordering::Relaxed),
        hidden_count: 0,
        dotfiles_hidden: false,
    }
}

/// Scan a drive. Returns `None` if cancelled.
pub fn scan(drive: &Drive, ctl: &ScanControl) -> Option<Tree> {
    let root_meta = meta(&drive.root)?;
    let pool = rayon::ThreadPoolBuilder::new()
        .stack_size(32 * 1024 * 1024)
        .build()
        .ok()?;
    let root = Arc::new(LiveDir::new(
        String::new(),
        root_meta.mtime,
        root_meta.created,
    ));
    *ctl.live() = Some(root.clone());
    pool.install(|| {
        scan_dir(
            &drive.root,
            root_meta.dev,
            ctl,
            &Chain {
                dir: &root,
                parent: None,
            },
        )
    });
    // Taking it back waits out any snapshot in progress, leaving the scan the only owner.
    ctl.live().take();
    if ctl.cancelled.load(Ordering::Relaxed) {
        return None;
    }
    let root = Arc::into_inner(root)?;
    let folder = pool.install(|| root.into_folder());
    Some(make_tree(drive, folder, ctl))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::fs::drive_for_path;

    #[test]
    fn pause_holds_the_scan_until_resumed_or_cancelled() {
        let dir = std::env::temp_dir().join(format!("sm_pause_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("a")).unwrap();
        fs::write(dir.join("a/f"), vec![1u8; 10]).unwrap();
        let drive = drive_for_path(&dir).unwrap();

        let ctl = Arc::new(ScanControl::default());
        ctl.set_paused(true);
        let (d, c) = (drive.clone(), ctl.clone());
        let t = std::thread::spawn(move || scan(&d, &c).map(|t| t.num_files));
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(!t.is_finished());
        assert_eq!(ctl.files.load(Ordering::Relaxed), 0);
        ctl.set_paused(false);
        assert_eq!(t.join().unwrap(), Some(1));

        let ctl = Arc::new(ScanControl::default());
        ctl.set_paused(true);
        let c = ctl.clone();
        let t = std::thread::spawn(move || scan(&drive, &c).is_none());
        std::thread::sleep(std::time::Duration::from_millis(100));
        ctl.cancel();
        assert!(t.join().unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

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
        let ai = tree
            .root
            .entries
            .iter()
            .position(|e| e.name == "a")
            .unwrap();
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
    fn entries_without_id_always_count() {
        // Windows gives no (device, inode): every folder must still be scanned.
        let ctl = ScanControl::default();
        assert!(ctl.first_sighting(None));
        assert!(ctl.first_sighting(None));
        assert!(ctl.first_sighting(Some((1, 2))));
        assert!(!ctl.first_sighting(Some((1, 2))));
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
