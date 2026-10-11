//! Commands the user can trigger from the toolbar, context menu or keyboard.

use crate::core::selection::Nav;

/// Something to do, whatever triggered it (key, menu, title bar icon, double-click).
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
    Settings,
    Nav(Nav),
}
