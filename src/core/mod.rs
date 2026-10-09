//! Everything that isn't drawing: the scanned tree, filesystem access, layout, camera and
//! selection. Nothing in here may depend on egui.

pub mod actions;
pub mod fs;
pub mod geometry;
pub mod layout;
pub mod model;
pub mod selection;
