//! A scan running on a background thread, with a second thread taking snapshots of it.

use super::{scan, Drive, ScanControl, ScanOptions};
use crate::constants::LIVE_SCAN_INTERVAL;
use crate::core::model::Tree;
use poll_promise::Promise;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub enum ScanStatus {
    Running,
    /// Finished; `None` if it was cancelled or failed.
    Finished(Option<Tree>),
}

pub struct ScanJob {
    pub drive: Drive,
    /// Progress counters and the cancel flag, shared with the scanner threads.
    pub ctl: Arc<ScanControl>,
    result: Promise<Option<Tree>>,
    /// The latest snapshot not yet handed out.
    latest: Arc<Mutex<Option<Tree>>>,
}

impl ScanJob {
    /// Start scanning `drive`. `on_update` runs on a background thread whenever a new snapshot
    /// or the result is ready.
    pub fn spawn(
        drive: Drive,
        opts: ScanOptions,
        on_update: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let ctl = Arc::new(ScanControl::new(opts));
        let latest = Arc::new(Mutex::new(None));
        let done = Arc::new(AtomicBool::new(false));
        let on_update = Arc::new(on_update);
        let (tx, result) = Promise::new();

        let (d, c, fin, notify) = (drive.clone(), ctl.clone(), done.clone(), on_update.clone());
        std::thread::spawn(move || {
            // A panicking scan counts as a failed one rather than taking the UI down with it.
            let tree = catch_unwind(AssertUnwindSafe(|| scan(&d, &c)))
                .ok()
                .flatten();
            fin.store(true, Ordering::Relaxed);
            tx.send(tree);
            notify();
        });

        // Snapshots are built here rather than on the UI thread, which only picks them up.
        let (d, c, out) = (drive.clone(), ctl.clone(), latest.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) && !c.cancelled.load(Ordering::Relaxed) {
                std::thread::sleep(LIVE_SCAN_INTERVAL);
                if let Some(t) = c.snapshot(&d) {
                    *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(t);
                    on_update();
                }
            }
        });

        Self {
            drive,
            ctl,
            result,
            latest,
        }
    }

    /// Check for the result. `Finished` hands the tree over, so it's returned only once.
    pub fn poll(&mut self) -> ScanStatus {
        match self.result.ready_mut() {
            None => ScanStatus::Running,
            Some(tree) => ScanStatus::Finished(tree.take()),
        }
    }

    /// The newest snapshot of what's been found so far, if there's one since the last call.
    pub fn take_snapshot(&self) -> Option<Tree> {
        self.latest.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub fn cancel(&self) {
        self.ctl.cancel();
    }
}
