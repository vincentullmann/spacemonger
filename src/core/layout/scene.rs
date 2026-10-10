//! The tree root plus layout settings: everything the geometry queries need.

use super::{content_of, locate, LayoutParams, Reshape};
use crate::core::geometry::Rect;
use crate::core::model::{Folder, Tree};

/// What's being laid out. `root` is `None` before a scan has finished, in which case every
/// query comes back empty.
#[derive(Clone, Copy)]
pub struct Scene<'a> {
    pub root: Option<&'a Folder>,
    pub params: LayoutParams,
}

impl<'a> Scene<'a> {
    pub fn new(tree: Option<&'a Tree>, params: LayoutParams) -> Self {
        Self {
            root: tree.map(|t| &t.root),
            params,
        }
    }

    /// See [`locate`].
    pub fn locate(&self, cam: Rect, path: &[usize], ovs: &[Reshape]) -> Option<Rect> {
        locate(self.root?, cam, path, self.params, ovs)
    }

    /// See [`content_of`].
    pub fn content_of(&self, cam: Rect, path: &[usize], ovs: &[Reshape]) -> Option<Rect> {
        content_of(self.root?, cam, path, self.params, ovs)
    }
}
