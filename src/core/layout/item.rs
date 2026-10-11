//! One laid-out box.

use super::FOLDER_FRAME;
use crate::core::geometry::Rect;
use std::rc::Rc;

/// A laid-out box: an entry, the free-space block, or an anonymous block of tiny entries.
pub struct Item {
    /// Index path (from the tree root) of the folder that owns this entry.
    pub folder: Rc<[usize]>,
    /// Entry index within `folder`; `None` for an unlabelled "too small" block.
    pub index: Option<usize>,
    /// Nesting depth from the scan root; -1 for the free-space block.
    pub depth: i32,
    pub is_folder: bool,
    pub is_free: bool,
    /// Big enough to carry a label (and, for folders, to show their contents).
    pub labeled: bool,
    /// Box in view coordinates, clipped to the view (plus a margin).
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Item {
    /// Full index path of the entry (folder path + index); `None` for an anonymous block.
    pub fn path(&self) -> Option<Vec<usize>> {
        let mut p = self.folder.to_vec();
        p.push(self.index?);
        Some(p)
    }

    /// The (clipped) box as an f64 rect.
    pub fn rect(&self) -> Rect {
        Rect::from_origin_size(
            (self.x as f64, self.y as f64),
            (self.w as f64, self.h as f64),
        )
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px > self.x && py > self.y && px < self.x + self.w && py < self.y + self.h
    }

    /// For folders: is the point on the frame / title bar (not the content area)?
    pub fn on_frame(&self, px: f32, py: f32, title_h: f32) -> bool {
        let frame = FOLDER_FRAME as f32;
        px < self.x + frame
            || py < self.y + title_h
            || px > self.x + self.w - frame
            || py > self.y + self.h - frame
    }
}
