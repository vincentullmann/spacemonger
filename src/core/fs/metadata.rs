//! The bits of file metadata the scanner needs, without following symlinks.

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub(super) struct Meta {
    pub is_dir: bool,
    pub is_file: bool,
    pub dev: u64,
    pub ino: u64,
    pub nlink: u64,
    pub len: u64,
    /// Allocated (on-disk) size.
    pub alloc: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: i64,
}

pub(super) fn meta(path: &Path) -> Option<Meta> {
    let m = fs::symlink_metadata(path).ok()?;
    let ft = m.file_type();
    let mtime = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    let alloc = filesize::file_real_size_fast(path, &m).unwrap_or(m.len());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(Meta {
            is_dir: ft.is_dir(),
            is_file: ft.is_file(),
            dev: m.dev(),
            ino: m.ino(),
            nlink: m.nlink(),
            len: m.len(),
            alloc,
            mtime,
        })
    }
    #[cfg(not(unix))]
    {
        Some(Meta {
            is_dir: ft.is_dir(),
            is_file: ft.is_file(),
            dev: 0,
            ino: 0,
            nlink: 1,
            len: m.len(),
            alloc,
            mtime,
        })
    }
}
