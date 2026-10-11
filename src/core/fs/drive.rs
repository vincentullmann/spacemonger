//! Scannable locations: mounted volumes or an arbitrary folder.

use super::FsError;
use lfs_core::{read_mounts, Mount, ReadOptions};
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

/// Real storage: has a size, sits on a disk (or is a network share), and isn't a bind mount
/// or a read-only image.
fn is_storage(m: &Mount) -> bool {
    m.stats().is_some_and(|s| s.size() > 0)
        && (m.disk.is_some() || m.is_remote() || m.info.fs_type == "zfs")
        && !m.info.bound
        && m.info.fs_type != "squashfs"
}

fn mounts() -> Vec<Mount> {
    read_mounts(&ReadOptions::default()).unwrap_or_default()
}

/// All mounted storage volumes.
pub fn volumes() -> Vec<Drive> {
    let mut seen = HashSet::new();
    let mut out: Vec<Drive> = mounts()
        .iter()
        .filter(|m| is_storage(m) && seen.insert(m.info.mount_point.clone()))
        .map(|m| {
            let root = m.info.mount_point.clone();
            Drive {
                name: root.display().to_string(),
                root,
                total: total(m),
                free: free(m),
                fs: m.info.fs_type.clone(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.root.cmp(&b.root));
    out
}

fn total(m: &Mount) -> u64 {
    m.stats().map_or(0, |s| s.size())
}

fn free(m: &Mount) -> u64 {
    m.stats().map_or(0, |s| s.available())
}

/// Build a `Drive` for an arbitrary folder, using the stats of the volume it lives on.
pub fn drive_for_path(path: &Path) -> Result<Drive, FsError> {
    let path = fs::canonicalize(path)
        .map(strip_verbatim)
        .map_err(|source| FsError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if !path.is_dir() {
        return Err(FsError::NotAFolder(path));
    }
    let mounts = mounts();
    let best = mounts
        .iter()
        .filter(|m| path.starts_with(&m.info.mount_point))
        .max_by_key(|m| m.info.mount_point.as_os_str().len());
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    Ok(Drive {
        name,
        total: best.map_or(0, total),
        free: best.map_or(0, free),
        fs: best.map_or(String::new(), |m| m.info.fs_type.clone()),
        root: path,
    })
}

/// On Windows `canonicalize` returns verbatim paths (`\\?\C:\dir`, `\\?\UNC\host\share`),
/// which never match the volume mount points (`C:\`) and look odd in the UI. Turn them back
/// into the ordinary form.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    if !cfg!(windows) {
        return path;
    }
    let plain = {
        let s = path.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            Some(PathBuf::from(format!(r"\\{rest}")))
        } else {
            s.strip_prefix(r"\\?\")
                .filter(|rest| rest.as_bytes().get(1) == Some(&b':'))
                .map(PathBuf::from)
        }
    };
    plain.unwrap_or(path)
}

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
