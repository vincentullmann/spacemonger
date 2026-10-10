//! Treemap layout (port of FolderView.buildFolderLayout / sizeFolders).
//!
//! Greedy binary split: entries (sorted largest first) are dealt into two
//! lists, keeping their sums as even as possible, and the rectangle is split
//! along its longer side in proportion. Recurse until a list holds one entry
//! or the rectangle is a few pixels wide. Boxes smaller than the density's
//! minimum label size are still real, hoverable items; they just get no label
//! and their folder contents aren't drawn.
//!
//! Geometry is f64 in view coordinates and always starts at the scan root, whose box is the
//! camera. Zooming moves the camera; each frame only the part inside the view is laid out.
//! A `Reshape` reshapes one folder's box (used to make a zoomed-in folder fill the window);
//! that folder is drawn after everything else so it sits on top of its neighbours.

mod content;
mod item;
mod neighbours;
mod params;
mod query;
mod reshape;
mod scene;
mod split;
mod treemap;

pub use content::{content, root_content};
pub use item::Item;
pub use neighbours::{neighbour, Dir};
pub use params::LayoutParams;
pub use query::{child_boxes, content_of, covering, hit_test, locate};
pub use reshape::Reshape;
pub use scene::Scene;
pub use treemap::build;

#[cfg(test)]
mod tests;
