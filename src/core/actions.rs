//! Commands the user can trigger from the toolbar, context menu or keyboard.

use crate::core::selection::Nav;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    Open,
    Reload,
    ZoomFull,
    ZoomIn,
    ZoomOut,
    ToggleFree,
    RunOpen,
    Delete,
    Hide,
    UnhideAll,
    ToggleDark,
    ClearSelection,
    Frame,
    Nav(Nav),
}
