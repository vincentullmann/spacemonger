//! In-memory tree of a scanned folder (port of CFolder / CFolderTree).

mod entry;
mod entry_ref;
mod folder;
mod tree;

#[cfg(test)]
pub mod testing;

pub use entry::{Entry, Kind};
pub use entry_ref::{path_after_remove, EntryRef};
pub use folder::Folder;
pub use tree::Tree;
