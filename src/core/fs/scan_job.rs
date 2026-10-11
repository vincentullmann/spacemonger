//! A scan running on a background thread, with a second thread taking snapshots of it.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use poll_promise::Promise;

use super::{scan, Drive, FsError, ScanControl, ScanOptions};
use crate::constants::LIVE_SCAN_INTERVAL;
use crate::core::model::Tree;

/// Where a [`ScanJob`] has got to.
pub enum ScanStatus {
    Running,
    /// Finished; `None` if it was cancelled.
    Finished(Option<Tree>),
    /// The scan couldn't start (unreadable root) or stopped unexpectedly.
    Failed(FsError),
}

/// What the scan thread hands back.
type ScanResult = Result<Option<Tree>, FsError>;

pub struct ScanJob {
    pub drive: Drive,
    /// Progress counters and the cancel flag, shared with the scanner threads.
    pub ctl: Arc<ScanControl>,
    result: Promise<ScanResult>,
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
                .unwrap_or(Err(FsError::ScanPanicked));
            fin.store(true, Ordering::Relaxed);
            tx.send(tree);
            notify();
        });

        // Snapshots are built here rather than on the UI thread, which only picks them up.
        let (d, c, out) = (drive.clone(), ctl.clone(), latest.clone());
        std::thread::spawn(move || {
            let mut last = None;
            while !done.load(Ordering::Relaxed) && !c.cancelled.load(Ordering::Relaxed) {
                std::thread::sleep(LIVE_SCAN_INTERVAL);
                // Nothing new found (paused, or a slow folder): keep the last snapshot.
                let found = c.found();
                if last == Some(found) {
                    continue;
                }
                last = Some(found);
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
            Some(result) => match std::mem::replace(result, Ok(None)) {
                Ok(tree) => ScanStatus::Finished(tree),
                Err(e) => ScanStatus::Failed(e),
            },
        }
    }

    /// 0..=1: bytes found so far against the volume's used space.
    pub fn progress(&self) -> f32 {
        let used = self.drive.total.saturating_sub(self.drive.free);
        if used == 0 {
            return 0.0;
        }
        (self.ctl.bytes.load(Ordering::Relaxed) as f32 / used as f32).min(1.0)
    }

    /// The newest snapshot of what's been found so far, if there's one since the last call.
    pub fn take_snapshot(&self) -> Option<Tree> {
        self.latest.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    /// Ask the scan to stop; its threads wind down on their own.
    pub fn cancel(&self) {
        self.ctl.cancel();
    }
}

impl Drop for ScanJob {
    /// A job nobody holds any more has nobody to report to: stop it.
    fn drop(&mut self) {
        self.cancel();
    }
}
