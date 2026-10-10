//! Errors shown to the user in the error dialog.

use crate::core::fs::FsError;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Fs(#[from] FsError),
    /// Running / opening an entry with the default handler failed.
    #[error("Cannot open {}:\n{source}", path.display())]
    Open { path: PathBuf, source: std::io::Error },
    /// Some entries couldn't be moved to the trash.
    #[error("Failed to move to trash:\n\n{}", trash_list(.0))]
    Trash(Vec<(PathBuf, trash::Error)>),
}

fn trash_list(failed: &[(PathBuf, trash::Error)]) -> String {
    failed.iter().map(|(p, e)| format!("{}:\n{e}", p.display())).collect::<Vec<_>>().join("\n\n")
}
