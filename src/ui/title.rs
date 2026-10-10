//! Window title: the selection, or the current folder, with sizes.

use crate::constants::APP_NAME;
use crate::core::model::Tree;
use crate::core::selection::Selection;
use crate::utils::format;

pub fn window_title(tree: Option<&Tree>, selection: &Selection, zoom: &[usize]) -> String {
    let Some(t) = tree else { return APP_NAME.to_string() };
    if selection.len() > 1 {
        let roots = selection.roots();
        let size: u64 = roots.iter().filter_map(|r| t.entry(r)).map(|e| e.size).sum();
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
    let size = if zoom.is_empty() { t.total_space } else { t.folder_at(zoom).map_or(0, |f| f.total) };
    format!(
        "{}  -  {} Total  -  {} Free  -  {APP_NAME}",
        t.full_path(zoom, None).display(),
        format::size_string(size, t.total_space, false),
        format::size_string(t.free_space, t.free_space, false),
    )
}
