//! Scannable locations: mounted volumes or an arbitrary folder.

use super::FsError;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// A scannable location (a mounted volume or an arbitrary folder).
#[derive(Clone, Debug)]
pub struct Drive {
    pub name: String,
    pub root: PathBuf,
    pub total: u64,
    pub free: u64,
    pub fs: String,
}

const KNOWN_PHYSICAL_FS: &[&str] = &[
    "afpfs", "apfs", "bcachefs", "btrfs", "cd9660", "cifs", "exfat", "ext2", "ext3", "ext4",
    "f2fs", "fat", "fat32", "fuseblk", "hfs", "hfsplus", "iso9660", "jfs", "msdos", "msdosfs",
    "nfs", "nfs4", "ntfs", "ntfs3", "reiserfs", "refs", "smbfs", "udf", "ufs", "vfat", "webdav",
    "xfs", "zfs",
];

fn is_physical(fs: &str) -> bool {
    let fs = fs.to_ascii_lowercase();
    KNOWN_PHYSICAL_FS.contains(&fs.as_str())
}

/// All mounted, non-virtual volumes.
pub fn volumes() -> Vec<Drive> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for d in disks.list() {
        let fs = d.file_system().to_string_lossy().to_string();
        if !is_physical(&fs) || d.total_space() == 0 {
            continue;
        }
        let root = d.mount_point().to_path_buf();
        if !seen.insert(root.clone()) {
            continue;
        }
        out.push(Drive {
            name: root.display().to_string(),
            root,
            total: d.total_space(),
            free: d.available_space(),
            fs,
        });
    }
    out.sort_by(|a, b| a.root.cmp(&b.root));
    out
}

/// Build a `Drive` for an arbitrary folder, using the stats of the volume it lives on.
pub fn drive_for_path(path: &Path) -> Result<Drive, FsError> {
    let path = fs::canonicalize(path).map_err(|source| FsError::Io { path: path.to_path_buf(), source })?;
    if !path.is_dir() {
        return Err(FsError::NotAFolder(path));
    }
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let best = disks
        .list()
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len());
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    Ok(Drive {
        name,
        total: best.map_or(0, |d| d.total_space()),
        free: best.map_or(0, |d| d.available_space()),
        fs: best.map_or(String::new(), |d| d.file_system().to_string_lossy().to_string()),
        root: path,
    })
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_for_path_errors() {
        let missing = Path::new("/definitely/not/here");
        assert!(matches!(drive_for_path(missing), Err(FsError::Io { .. })));
        let file = std::env::temp_dir().join(format!("sm_test_file_{}", std::process::id()));
        fs::write(&file, b"x").unwrap();
        let err = drive_for_path(&file).unwrap_err();
        assert!(err.to_string().starts_with("Not a folder: "));
        fs::remove_file(&file).unwrap();
    }
}
