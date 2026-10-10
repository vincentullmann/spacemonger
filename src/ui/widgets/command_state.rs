//! What the command buttons need to know to enable themselves.

/// Snapshot of app state for enabling toolbar buttons and menu items.
pub struct CommandState {
    pub has_tree: bool,
    pub zoomed: bool,
    /// The primary selection is a folder.
    pub sel_folder: bool,
    pub has_sel: bool,
    pub show_free: bool,
    /// Number of hidden entries.
    pub hidden: usize,
    pub dark: bool,
}
