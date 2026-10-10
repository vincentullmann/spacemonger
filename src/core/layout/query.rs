//! Geometry queries against the layout: where is an entry, what's under a point.

use super::split::{halves, weight};
use super::{content, root_content, Item, LayoutParams, Reshape};
use crate::core::geometry::{Rect, RectExt, Size};
use crate::core::model::Folder;

/// Boxes of the folder's laid-out entries (not hidden or empty) when its content area is
/// `area`: the plain split, with no clipping or size limits.
pub fn child_boxes(folder: &Folder, area: Rect, p: LayoutParams) -> Vec<(usize, Rect)> {
    fn walk(folder: &Folder, list: &[usize], r: Rect, p: LayoutParams, out: &mut Vec<(usize, Rect)>) {
        let Some(halves) = halves(folder, list, r, p) else { return };
        for (l, r) in halves {
            match l.len() {
                0 => {}
                1 => out.push((l[0], r)),
                _ => walk(folder, &l, r, p, out),
            }
        }
    }
    let mut out = Vec::new();
    let all: Vec<usize> = (0..folder.entries.len()).collect();
    walk(folder, &all, area, p, &mut out);
    out
}

/// Unclipped box of the entry at `path` (from the scan root) with the root's box at `cam`,
/// as drawn with overrides `ovs`. Pure geometry: ignores size thresholds, so it also works for
/// boxes too small to draw. An empty path gives `cam` itself.
pub fn locate(root: &Folder, cam: Rect, path: &[usize], p: LayoutParams, ovs: &[Reshape]) -> Option<Rect> {
    let mut folder = root;
    let mut area = root_content(cam);
    let mut b = cam;
    for (k, &target) in path.iter().enumerate() {
        weight(folder.entries.get(target)?, p).checked_sub(1)?; // not laid out if empty / hidden
        let mut list: Vec<usize> = (0..folder.entries.len()).collect();
        let mut r = area;
        loop {
            let [(l1, r1), (l2, r2)] = halves(folder, &list, r, p)?;
            if l2.is_empty() && l1.len() == 1 {
                r = r1;
                break;
            }
            (list, r) = if l1.contains(&target) {
                (l1, r1)
            } else if l2.contains(&target) {
                (l2, r2)
            } else {
                return None;
            };
            if list.len() == 1 {
                break;
            }
        }
        if let Some(o) = ovs.iter().find(|o| o.path[..] == path[..=k]) {
            r = o.apply(r);
        }
        b = r;
        if k + 1 < path.len() {
            folder = folder.entries[target].child()?;
            area = content(r);
        }
    }
    Some(b)
}

/// Content area of the folder at `path` with the root's box at `cam`.
pub fn content_of(root: &Folder, cam: Rect, path: &[usize], p: LayoutParams, ovs: &[Reshape]) -> Option<Rect> {
    if path.is_empty() {
        Some(root_content(cam))
    } else {
        locate(root, cam, path, p, ovs).map(content)
    }
}

/// Deepest folder whose content area covers the whole view.
pub fn covering(items: &[Item], view: Size) -> Vec<usize> {
    let view = root_content(view.to_rect());
    items
        .iter()
        .filter(|it| it.is_folder && it.labeled && it.index.is_some())
        .filter(|it| content(it.rect()).covers(&view))
        .max_by_key(|it| it.folder.len())
        .and_then(Item::path)
        .unwrap_or_default()
}

/// Item under the point, ignoring folder content areas, anonymous blocks and free space.
/// Searches from the end, so reshaped folders (drawn last, on top) win over what's beneath.
pub fn hit_test(items: &[Item], px: f32, py: f32) -> Option<usize> {
    // The last item containing the point is the deepest one on top.
    let i = items.iter().rposition(|it| it.contains(px, py))?;
    let it = &items[i];
    let in_content = it.is_folder && it.labeled && !it.on_frame(px, py);
    (it.index.is_some() && !it.is_free && !in_content).then_some(i)
}
