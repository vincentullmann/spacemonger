//! What the scanner threads share with the UI: options, progress counters, pause / cancel,
//! and the tree found so far.

use super::live::LiveDir;
use super::scanner::make_tree;
use super::Drive;
use crate::core::model::Tree;
use dashmap::DashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

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
    /// Folders and entries that couldn't be read (permissions, vanished mid-scan, ...).
    pub errors: AtomicU64,
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
            errors: AtomicU64::new(0),
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
    pub(super) fn is_cancelled(&self) -> bool {
        while self.is_paused() && !self.cancelled.load(Ordering::Relaxed) {
            std::thread::sleep(PAUSE_POLL);
        }
        self.cancelled.load(Ordering::Relaxed)
    }
    /// Returns false if this (device, inode) was already counted (hard links, bind mounts).
    /// Entries without an id are always counted.
    pub(super) fn first_sighting(&self, id: Option<(u64, u64)>) -> bool {
        id.is_none_or(|id| self.seen.insert(id))
    }
    /// (files, folders, bytes) found so far: changes whenever the scan finds anything.
    pub fn found(&self) -> (u64, u64, u64) {
        (
            self.files.load(Ordering::Relaxed),
            self.folders.load(Ordering::Relaxed),
            self.bytes.load(Ordering::Relaxed),
        )
    }

    /// Count something that couldn't be read.
    pub(super) fn failed(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }
    pub(super) fn live(&self) -> MutexGuard<'_, Option<Arc<LiveDir>>> {
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
