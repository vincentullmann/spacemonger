//! Window title: the selection, or the current folder, with sizes.

use crate::constants::APP_NAME;
use crate::core::fs::ScanJob;
use crate::core::model::Tree;
use crate::core::selection::Selection;
use crate::utils::format;
use num_format::{Locale, ToFormattedString};
use std::sync::atomic::Ordering;

pub fn window_title(tree: Option<&Tree>, selection: &Selection, zoom: &[usize]) -> String {
    let Some(t) = tree else {
        return APP_NAME.to_string();
    };
    if selection.len() > 1 {
        let roots = selection.roots();
        let size: u64 = roots
            .iter()
            .filter_map(|r| t.entry(r))
            .map(|e| e.size)
            .sum();
        return format!(
            "{} items selected  -  {}  -  {}  -  {APP_NAME}",
            roots.len(),
            format::size_string(size, t.total_space, true),
            format::size_string(size, t.total_space, false),
        );
    }
    if let Some((e, r)) = selection.primary().and_then(|r| Some((t.entry(r)?, r))) {
        return format!(
            "{}  -  {}  -  {}  -  {APP_NAME}",
            t.path_of(r).display(),
            format::size_string(e.size, t.total_space, true),
            format::size_string(e.size, t.total_space, false),
        );
    }
    let size = if zoom.is_empty() {
        t.total_space
    } else {
        t.folder_at(zoom).map_or(0, |f| f.total)
    };
    format!(
        "{}  -  {} Total  -  {} Free  -  {APP_NAME}",
        t.full_path(zoom, None).display(),
        format::size_string(size, t.total_space, false),
        format::size_string(t.free_space, t.free_space, false),
    )
}

/// While scanning: what's been found so far, and how far along it is (bytes found against the
/// volume's used space, as in the scan dialog).
pub fn scan_title(job: &ScanJob) -> (String, f32) {
    let ctl = &job.ctl;
    let bytes = ctl.bytes.load(Ordering::Relaxed);
    let files = ctl.files.load(Ordering::Relaxed);
    let used = job.drive.total.saturating_sub(job.drive.free);
    let frac = if used > 0 {
        (bytes as f32 / used as f32).min(1.0)
    } else {
        0.0
    };
    let verb = if ctl.is_paused() {
        "Paused"
    } else {
        "Scanning"
    };
    let text = format!(
        "{verb} {}  -  {} files  -  {}  -  {APP_NAME}",
        job.drive.root.display(),
        files.to_formatted_string(&Locale::en),
        format::size_string(bytes, 0, false),
    );
    (text, frac)
}
