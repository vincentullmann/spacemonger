//! One file, folder or free-space block.

use super::Folder;

pub enum Kind {
    File,
    Dir(Box<Folder>),
    /// Synthetic "free space" block at the root.
    Free,
}

pub struct Entry {
    pub name: String,
    /// Allocated (on-disk) size; used for layout.
    pub size: u64,
    /// Logical size; shown to the user.
    pub actual: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: i64,
    pub kind: Kind,
    /// Hidden from the view by the user (its size is already subtracted from ancestors).
    pub hidden: bool,
}

impl Entry {
    pub fn child(&self) -> Option<&Folder> {
        match &self.kind {
            Kind::Dir(f) => Some(f),
            _ => None,
        }
    }
}
