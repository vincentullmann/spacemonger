//! Parallel scanner: by default stays on one filesystem and counts hard links once.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use rayon::prelude::*;

use super::control::ScanControl;
use super::live::{Chain, LiveDir};
use super::metadata::{entry_meta, meta};
use super::{Drive, FsError};
use crate::core::model::{Entry, Folder, Kind, Tree};

/// Stack size of the scanner threads: `scan_dir` recurses once per folder level.
const SCAN_STACK: usize = 32 * 1024 * 1024;

/// Scan the folder at `path` into `chain.dir`.
fn scan_dir(path: &Path, dev: u64, ctl: &ScanControl, chain: &Chain) {
    if ctl.is_cancelled() {
        return;
    }
    if let Ok(mut cur) = ctl.current.try_lock() {
        *cur = path.display().to_string();
    }

    let Ok(rd) = fs::read_dir(path) else {
        ctl.failed();
        return;
    };
    let mut listed: Vec<fs::DirEntry> = rd
        .filter_map(|e| e.inspect_err(|_| ctl.failed()).ok())
        .collect();
    listed.sort_by_key(fs::DirEntry::file_name);

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut dir_paths = Vec::new();
    for item in listed {
        if ctl.is_cancelled() {
            break;
        }
        let child = item.path();
        let Ok(m) = entry_meta(&item, &child) else {
            ctl.failed();
            continue;
        };
        let name = item.file_name().into_boxed_os_str();
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
pub(super) fn make_tree(drive: &Drive, mut root: Folder, ctl: &ScanControl) -> Tree {
    root.entries.push(Entry::new("", drive.free, Kind::Free));
    root.finalize();
    Tree {
        root,
        root_path: drive.root.clone(),
        total_space: drive.total,
        free_space: drive.free,
        num_files: ctl.files.load(Ordering::Relaxed),
        num_folders: ctl.folders.load(Ordering::Relaxed),
        unreadable: ctl.errors.load(Ordering::Relaxed),
        hidden_count: 0,
        dotfiles_hidden: false,
    }
}

/// Scan a drive. `Ok(None)` if cancelled; an error if its root can't be read at all.
pub fn scan(drive: &Drive, ctl: &ScanControl) -> Result<Option<Tree>, FsError> {
    let io_err = |source| FsError::Io {
        path: drive.root.clone(),
        source,
    };
    let root_meta = meta(&drive.root).map_err(io_err)?;
    let pool = rayon::ThreadPoolBuilder::new()
        .stack_size(SCAN_STACK)
        .build()
        .map_err(|e| io_err(std::io::Error::other(e)))?;
    let root = Arc::new(LiveDir::new(
        OsStr::new("").into(),
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
        return Ok(None);
    }
    // The scan threads and the snapshot are done with it, so this is the only reference.
    let Some(root) = Arc::into_inner(root) else {
        return Ok(None);
    };
    let folder = pool.install(|| root.into_folder());
    Ok(Some(make_tree(drive, folder, ctl)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::fs::drive_for_path;

    /// `n` bytes that don't compress, so allocated sizes stay close to `n` on compressing
    /// filesystems (btrfs, ZFS) too.
    fn noise(n: usize) -> Vec<u8> {
        let mut x: u32 = 0x9e37_79b9;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect()
    }

    /// Index of the entry called `name` in `folder`.
    fn index_of(tree: &Tree, folder: &[usize], name: &str) -> usize {
        let f = tree.folder_at(folder).unwrap();
        f.entries
            .iter()
            .position(|e| e.display_name() == name)
            .unwrap()
    }

    #[test]
    fn pause_holds_the_scan_until_resumed_or_cancelled() {
        let dir = std::env::temp_dir().join(format!("sm_pause_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("a")).unwrap();
        fs::write(dir.join("a/f"), noise(10)).unwrap();
        let drive = drive_for_path(&dir).unwrap();

        let ctl = Arc::new(ScanControl::default());
        ctl.set_paused(true);
        let (d, c) = (drive.clone(), ctl.clone());
        let t = std::thread::spawn(move || scan(&d, &c).unwrap().map(|t| t.num_files));
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(!t.is_finished());
        assert_eq!(ctl.files.load(Ordering::Relaxed), 0);
        ctl.set_paused(false);
        assert_eq!(t.join().unwrap(), Some(1));

        let ctl = Arc::new(ScanControl::default());
        ctl.set_paused(true);
        let c = ctl.clone();
        let t = std::thread::spawn(move || scan(&drive, &c).unwrap().is_none());
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
        fs::write(dir.join("a/b/big"), noise(100_000)).unwrap();
        fs::write(dir.join("a/small"), noise(10)).unwrap();
        fs::write(dir.join("top"), noise(50_000)).unwrap();

        let drive = drive_for_path(&dir).unwrap();
        let ctl = ScanControl::default();
        let mut tree = scan(&drive, &ctl).unwrap().unwrap();
        assert_eq!(tree.num_files, 3);
        assert_eq!(tree.num_folders, 2);

        let ai = index_of(&tree, &[], "a");
        let bi = index_of(&tree, &[ai], "b");
        let a_size = tree.root.entries[ai].size;
        assert!(a_size >= 100_000);
        assert_eq!(
            tree.full_path(&[ai], Some(bi)),
            dir.canonicalize().unwrap().join("a").join("b")
        );

        // remove a/b
        let b_size = tree.entry_at(&[ai], bi).unwrap().size;
        let root_total = tree.root.total;
        tree.remove(&[ai], bi);
        assert_eq!(tree.root.total, root_total - b_size);
        assert_eq!(tree.root.entries[ai].size, a_size - b_size);
        assert_eq!(tree.folder_at(&[ai]).unwrap().entries.len(), 1);
        assert_eq!((tree.num_files, tree.num_folders), (2, 1));

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
        fs::write(dir.join("f"), noise(10_000)).unwrap();
        fs::hard_link(dir.join("f"), dir.join("sub/g")).unwrap();

        let tree = scan(&drive_for_path(&dir).unwrap(), &ScanControl::default())
            .unwrap()
            .unwrap();
        assert_eq!(tree.num_files, 1);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_names_keep_their_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let dir = std::env::temp_dir().join(format!("sm_test_bytes_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // Two names that only differ in invalid bytes: both lossy forms are "a\u{fffd}".
        for name in [&b"a\xff"[..], &b"a\xfe"[..]] {
            fs::write(dir.join(OsStr::from_bytes(name)), noise(4096)).unwrap();
        }

        let tree = scan(&drive_for_path(&dir).unwrap(), &ScanControl::default())
            .unwrap()
            .unwrap();
        let files: Vec<_> = tree
            .root
            .entries
            .iter()
            .filter(|e| matches!(e.kind, Kind::File))
            .collect();
        assert_eq!(files.len(), 2);
        // Each path leads back to its own file.
        for i in 0..tree.root.entries.len() {
            if matches!(tree.root.entries[i].kind, Kind::File) {
                assert!(tree.full_path(&[], Some(i)).exists());
            }
        }
        assert_ne!(files[0].name, files[1].name);
        assert_eq!(files[0].display_name(), files[1].display_name());

        fs::remove_dir_all(&dir).unwrap();
    }
}
