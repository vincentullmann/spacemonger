//! Panels and popups: title bar, window frame, dialog frame, path bar, context menu, info tip.

pub mod buttons;
mod command_state;
mod context_menu;
pub mod dialog;
mod infotip;
mod menu;
mod path_bar;
pub mod titlebar;
mod window_drag;
mod window_frame;

pub use command_state::CommandState;
pub use context_menu::context_menu;
pub use infotip::infotip;
pub use path_bar::path_bar;
pub use titlebar::titlebar;
pub use window_drag::{move_on_drag, wm_grab};
pub use window_frame::window_frame;

/// For lists of items (the settings sidebar, drop-down entries, menus): hovering an item
/// fills it without the outline buttons get.
pub fn flat_items(ui: &mut eframe::egui::Ui) {
    let w = &mut ui.visuals_mut().widgets;
    w.hovered.bg_stroke = eframe::egui::Stroke::NONE;
    w.active.bg_stroke = eframe::egui::Stroke::NONE;
}
