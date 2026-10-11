//! The bits of file metadata the scanner needs, without following symlinks.

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub(super) struct Meta {
    pub is_dir: bool,
    pub is_file: bool,
    /// Filesystem the entry lives on. Always 0 off Unix: there, crossing onto another volume
    /// means going through a mount point (a reparse point), which is never descended.
    pub dev: u64,
    /// (device, inode), identifying the file across hard links and bind mounts. `None` where
    /// the platform doesn't give it cheaply (Windows), which turns off de-duplication.
    pub id: Option<(u64, u64)>,
    pub nlink: u64,
    pub len: u64,
    /// Allocated (on-disk) size.
    pub alloc: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: i64,
    /// Creation time, seconds since the Unix epoch; 0 if not recorded.
    pub created: i64,
}

pub(super) fn meta(path: &Path) -> Option<Meta> {
    let m = fs::symlink_metadata(path).ok()?;
    let ft = m.file_type();
    let secs = |t: std::io::Result<std::time::SystemTime>| {
        t.ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64)
    };
    let (mtime, created) = (secs(m.modified()), secs(m.created()));
    let alloc = filesize::file_real_size_fast(path, &m).unwrap_or(m.len());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(Meta {
            is_dir: ft.is_dir(),
            is_file: ft.is_file(),
            dev: m.dev(),
            id: Some((m.dev(), m.ino())),
            nlink: m.nlink(),
            len: m.len(),
            alloc,
            mtime,
            created,
        })
    }
    #[cfg(not(unix))]
    {
        Some(Meta {
            is_dir: ft.is_dir(),
            is_file: ft.is_file(),
            dev: 0,
            id: None,
            nlink: 1,
            len: m.len(),
            alloc,
            mtime,
            created,
        })
    }
}
