//! Filesystem access: volumes and the parallel scanner (port of FileSystems / fs/*).

mod drive;
mod metadata;
mod scanner;

pub use drive::{drive_for_path, volumes, Drive};
pub use scanner::{scan, ScanControl};
