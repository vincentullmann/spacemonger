//! Right-click menu on the map.

use super::menu::CommandMenu;
use super::CommandState;
use crate::core::actions::Action;
use crate::ui::keymap::{Command, Keymap};
use eframe::egui;

/// The menu for `resp` (the map); returns the command picked.
pub fn context_menu(resp: &egui::Response, st: &CommandState, keys: &Keymap) -> Option<Action> {
    let mut menu = CommandMenu::new(keys, st, true);
    resp.context_menu(|ui| {
        super::flat_items(ui);
        menu.items(ui, &[Command::ZoomIn, Command::ZoomOut, Command::ZoomFull]);
        ui.separator();
        menu.items(
            ui,
            &[
                Command::RunOpen,
                Command::Delete,
                Command::Hide,
                Command::UnhideAll,
            ],
        );
        ui.separator();
        menu.items(ui, &[Command::Open, Command::Reload, Command::ToggleFree]);
    });
    menu.clicked
}
