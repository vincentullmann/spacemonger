//! Filesystem errors.

use std::path::PathBuf;

/// What can go wrong opening or scanning a location.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("Cannot open {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Not a folder: {}", .0.display())]
    NotAFolder(PathBuf),
    #[error("The scan stopped unexpectedly")]
    ScanPanicked,
}
