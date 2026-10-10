//! In-memory tree of a scanned folder (port of CFolder / CFolderTree).

mod entry;
mod entry_ref;
mod folder;
mod tree;

pub use entry::{Entry, Kind};
pub use entry_ref::EntryRef;
pub use folder::Folder;
pub use tree::Tree;
