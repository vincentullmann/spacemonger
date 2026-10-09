//! Filesystem model and parallel scanner (port of CFolder / CFolderTree / FileSystems).

use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

pub enum Kind {
    File,
    Dir(Box<Folder>),
    /// Synthetic "free space" block at the root.
    Free,
}

pub struct Entry {
    pub name: String,
    /// Allocated (on-disk) size; used for layout.
    pub size: u64,
    /// Logical size; shown to the user.
    pub actual: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: i64,
    pub kind: Kind,
}

impl Entry {
    pub fn child(&self) -> Option<&Folder> {
        match &self.kind {
            Kind::Dir(f) => Some(f),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Folder {
    /// Sorted by `size`, descending.
    pub entries: Vec<Entry>,
    pub total: u64,
}

impl Folder {
    fn finalize(&mut self) {
        // Stable sort keeps name order for equal sizes.
        self.entries.sort_by_key(|e| std::cmp::Reverse(e.size));
        self.total = self.entries.iter().map(|e| e.size).sum();
    }
}

/// A scannable location (a mounted volume or an arbitrary folder).
#[derive(Clone, Debug)]
pub struct Drive {
    pub name: String,
    pub root: PathBuf,
    pub total: u64,
    pub free: u64,
    pub fs: String,
}

pub struct Tree {
    pub root: Folder,
    pub root_path: PathBuf,
    pub total_space: u64,
    pub free_space: u64,
    pub num_files: u64,
    pub num_folders: u64,
}

impl Tree {
    pub fn folder_at(&self, path: &[usize]) -> Option<&Folder> {
        let mut f = &self.root;
        for &i in path {
            f = f.entries.get(i)?.child()?;
        }
        Some(f)
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

// ---------------------------------------------------------------------------
// Volumes

const KNOWN_PHYSICAL_FS: &[&str] = &[
    "afpfs", "apfs", "bcachefs", "btrfs", "cd9660", "cifs", "exfat", "ext2", "ext3", "ext4",
    "f2fs", "fat", "fat32", "fuseblk", "hfs", "hfsplus", "iso9660", "jfs", "msdos", "msdosfs",
    "nfs", "nfs4", "ntfs", "ntfs3", "reiserfs", "refs", "smbfs", "udf", "ufs", "vfat", "webdav",
    "xfs", "zfs",
];

fn is_physical(fs: &str) -> bool {
    let fs = fs.to_ascii_lowercase();
    KNOWN_PHYSICAL_FS.contains(&fs.as_str())
}

/// All mounted, non-virtual volumes.
pub fn volumes() -> Vec<Drive> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for d in disks.list() {
        let fs = d.file_system().to_string_lossy().to_string();
        if !is_physical(&fs) || d.total_space() == 0 {
            continue;
        }
        let root = d.mount_point().to_path_buf();
        if !seen.insert(root.clone()) {
            continue;
        }
        out.push(Drive {
            name: root.display().to_string(),
            root,
            total: d.total_space(),
            free: d.available_space(),
            fs,
        });
    }
    out.sort_by(|a, b| a.root.cmp(&b.root));
    out
}

/// Build a `Drive` for an arbitrary folder, using the stats of the volume it lives on.
pub fn drive_for_path(path: &Path) -> Option<Drive> {
    let path = fs::canonicalize(path).ok()?;
    if !path.is_dir() {
        return None;
    }
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let best = disks
        .list()
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len());
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    Some(Drive {
        name,
        total: best.map_or(0, |d| d.total_space()),
        free: best.map_or(0, |d| d.available_space()),
        fs: best.map_or(String::new(), |d| d.file_system().to_string_lossy().to_string()),
        root: path,
    })
}

// ---------------------------------------------------------------------------
// Scanning

/// Shared state between the scanner threads and the UI.
pub struct ScanControl {
    pub cancelled: AtomicBool,
    pub files: AtomicU64,
    pub folders: AtomicU64,
    pub bytes: AtomicU64,
    pub current: Mutex<String>,
    seen: Vec<Mutex<HashSet<(u64, u64)>>>,
}

impl Default for ScanControl {
    fn default() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            files: AtomicU64::new(0),
            folders: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            current: Mutex::new(String::new()),
            seen: (0..64).map(|_| Mutex::new(HashSet::new())).collect(),
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
        let shard = (ino.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 58) as usize;
        self.seen[shard].lock().unwrap().insert((dev, ino))
    }
}

struct Meta {
    is_dir: bool,
    is_file: bool,
    dev: u64,
    ino: u64,
    nlink: u64,
    len: u64,
    alloc: u64,
    mtime: i64,
}

fn meta(path: &Path) -> Option<Meta> {
    let m = fs::symlink_metadata(path).ok()?;
    let ft = m.file_type();
    let mtime = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(Meta {
            is_dir: ft.is_dir(),
            is_file: ft.is_file(),
            dev: m.dev(),
            ino: m.ino(),
            nlink: m.nlink(),
            len: m.len(),
            alloc: m.blocks() * 512,
            mtime,
        })
    }
    #[cfg(not(unix))]
    {
        Some(Meta {
            is_dir: ft.is_dir(),
            is_file: ft.is_file(),
            dev: 0,
            ino: 0,
            nlink: 1,
            len: m.len(),
            alloc: m.len(),
            mtime,
        })
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
    });
    root.finalize();
    Some(Tree {
        root,
        root_path: drive.root.clone(),
        total_space: drive.total,
        free_space: drive.free,
        num_files: ctl.files.load(Ordering::Relaxed),
        num_folders: ctl.folders.load(Ordering::Relaxed),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
