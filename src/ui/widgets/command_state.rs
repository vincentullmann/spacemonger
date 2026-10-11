//! What the command buttons need to know to enable themselves.

use crate::ui::keymap::Command;

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
}

impl CommandState {
    /// Whether `cmd` can run now (menus and buttons grey out otherwise).
    pub fn enabled(&self, cmd: Command) -> bool {
        match cmd {
            Command::Reload | Command::ToggleFree => self.has_tree,
            Command::ZoomIn => self.has_tree && self.sel_folder,
            Command::ZoomOut | Command::ZoomFull => self.has_tree && self.zoomed,
            Command::RunOpen | Command::Delete | Command::Hide => self.has_sel,
            Command::UnhideAll => self.hidden > 0,
            _ => true,
        }
    }
}
