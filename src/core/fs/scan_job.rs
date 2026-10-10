//! A scan running on a background thread.

use super::{scan, Drive, ScanControl};
use crate::core::model::Tree;
use std::sync::{mpsc, Arc};

pub enum ScanStatus {
    Running,
    /// Finished; `None` if it was cancelled or failed.
    Finished(Option<Tree>),
}

pub struct ScanJob {
    pub drive: Drive,
    /// Progress counters and the cancel flag, shared with the scanner threads.
    pub ctl: Arc<ScanControl>,
    rx: mpsc::Receiver<Option<Tree>>,
}

impl ScanJob {
    /// Start scanning `drive`; `on_done` runs on the scan thread when it finishes.
    pub fn spawn(drive: Drive, on_done: impl FnOnce() + Send + 'static) -> Self {
        let ctl = Arc::new(ScanControl::default());
        let (tx, rx) = mpsc::channel();
        let (d, c) = (drive.clone(), ctl.clone());
        std::thread::spawn(move || {
            let res = scan(&d, &c);
            let _ = tx.send(res);
            on_done();
        });
        Self { drive, ctl, rx }
    }

    pub fn poll(&self) -> ScanStatus {
        match self.rx.try_recv() {
            Ok(tree) => ScanStatus::Finished(tree),
            Err(mpsc::TryRecvError::Disconnected) => ScanStatus::Finished(None),
            Err(mpsc::TryRecvError::Empty) => ScanStatus::Running,
        }
    }

    pub fn cancel(&self) {
        self.ctl.cancel();
    }
}
