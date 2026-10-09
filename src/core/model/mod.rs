//! In-memory tree of a scanned folder (port of CFolder / CFolderTree).

mod entry;
mod folder;
mod tree;

pub use entry::{Entry, Kind};
pub use folder::Folder;
pub use tree::Tree;
