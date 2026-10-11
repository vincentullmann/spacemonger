//! One file, folder or free-space block.

use std::borrow::Cow;
use std::ffi::OsStr;

use super::Folder;

pub enum Kind {
    File,
    Dir(Box<Folder>),
    /// Synthetic "free space" block at the root.
    Free,
}

pub struct Entry {
    /// The name on disk, as is (not necessarily UTF-8); see [`Entry::display_name`].
    pub name: Box<OsStr>,
    /// Allocated (on-disk) size; used for layout.
    pub size: u64,
    /// Logical size; shown to the user.
    pub actual: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: i64,
    /// Creation time, seconds since the Unix epoch; 0 where the filesystem doesn't record it.
    pub created: i64,
    pub kind: Kind,
    /// Hidden from the view by the user (its size is already subtracted from ancestors).
    pub hidden: bool,
}

impl Entry {
    /// An entry with both sizes `size` and no times: the free-space block, and tests.
    pub fn new(name: impl AsRef<OsStr>, size: u64, kind: Kind) -> Self {
        Self {
            name: name.as_ref().into(),
            size,
            actual: size,
            mtime: 0,
            created: 0,
            kind,
            hidden: false,
        }
    }

    /// The name for showing: as is when it's valid UTF-8, otherwise with the bad bytes
    /// replaced (borrowed, so no allocation in the common case).
    pub fn display_name(&self) -> Cow<'_, str> {
        self.name.to_string_lossy()
    }

    /// Named `.something` (hidden by convention on Unix).
    pub fn is_dotfile(&self) -> bool {
        self.name.as_encoded_bytes().starts_with(b".")
    }

    /// The folder's contents, for a folder.
    pub fn child(&self) -> Option<&Folder> {
        match &self.kind {
            Kind::Dir(f) => Some(f),
            _ => None,
        }
    }

    /// (files, folders) this entry stands for: itself and, for a folder, everything in it.
    pub fn counts(&self) -> (u64, u64) {
        match &self.kind {
            Kind::File => (1, 0),
            Kind::Free => (0, 0),
            Kind::Dir(f) => f
                .entries
                .iter()
                .map(Entry::counts)
                .fold((0, 1), |(a, b), (c, d)| (a + c, b + d)),
        }
    }
}
