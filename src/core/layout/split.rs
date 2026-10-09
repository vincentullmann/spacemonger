//! The greedy binary split shared by layout and the geometry queries.

use super::LayoutParams;
use crate::core::geometry::Rect;
use crate::core::model::{Entry, Folder, Kind};

pub(super) fn weight(e: &Entry, p: LayoutParams) -> u64 {
    if e.hidden || (matches!(e.kind, Kind::Free) && !p.show_free) {
        0
    } else {
        e.size
    }
}

/// One greedy split step: deal entries (largest first) into two lists with sums as even as
/// possible, and split the rectangle along its longer side in proportion.
/// Scale-invariant, so `locate` can follow it at any zoom.
pub(super) fn halves(folder: &Folder, indices: &[usize], r: Rect, p: LayoutParams) -> Option<[(Vec<usize>, Rect); 2]> {
    let mut l1 = Vec::new();
    let mut l2 = Vec::new();
    let (mut s1, mut s2) = (0u128, 0u128);
    for &i in indices {
        let size = weight(&folder.entries[i], p);
        if size == 0 {
            continue;
        }
        if s1 <= s2 {
            l1.push(i);
            s1 += size as u128;
        } else {
            l2.push(i);
            s2 += size as u128;
        }
    }
    if s1 + s2 == 0 {
        return None;
    }

    let (wbias, hbias) = match p.bias {
        b if b > 0 => ((b + 8) as f64, 8.0),
        b if b < 0 => (8.0, (-b + 8) as f64),
        _ => (8.0, 8.0),
    };
    let f = s1 as f64 / (s1 + s2) as f64;
    let (r1, r2) = if r.w * wbias > r.h * hbias {
        let sp = r.w.max(0.0) * f;
        (Rect::new(r.x, r.y, sp, r.h), Rect::new(r.x + sp, r.y, r.w - sp, r.h))
    } else {
        let sp = r.h.max(0.0) * f;
        (Rect::new(r.x, r.y, r.w, sp), Rect::new(r.x, r.y + sp, r.w, r.h - sp))
    };
    Some([(l1, r1), (l2, r2)])
}
