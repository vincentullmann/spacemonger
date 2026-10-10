//! A scan running on a background thread.

use super::{scan, Drive, ScanControl};
use crate::core::model::Tree;
use poll_promise::Promise;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

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
}

impl ScanJob {
    /// Start scanning `drive`; `on_done` runs on the scan thread once the result is ready.
    pub fn spawn(drive: Drive, on_done: impl FnOnce() + Send + 'static) -> Self {
        let ctl = Arc::new(ScanControl::default());
        let (tx, result) = Promise::new();
        let (d, c) = (drive.clone(), ctl.clone());
        std::thread::spawn(move || {
            // A panicking scan counts as a failed one rather than taking the UI down with it.
            let tree = catch_unwind(AssertUnwindSafe(|| scan(&d, &c))).ok().flatten();
            tx.send(tree);
            on_done();
        });
        Self { drive, ctl, result }
    }

    /// Check for the result. `Finished` hands the tree over, so it's returned only once.
    pub fn poll(&mut self) -> ScanStatus {
        match self.result.ready_mut() {
            None => ScanStatus::Running,
            Some(tree) => ScanStatus::Finished(tree.take()),
        }
    }

    pub fn cancel(&self) {
        self.ctl.cancel();
    }
}
