//! Filesystem access: volumes and the parallel scanner (port of FileSystems / fs/*).

mod drive;
mod error;
mod live;
mod metadata;
mod scan_job;
mod scanner;

pub use drive::{drive_for_path, volumes, Drive};
pub use error::FsError;
pub use scan_job::{ScanJob, ScanStatus};
pub use scanner::{scan, ScanControl};
