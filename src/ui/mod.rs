//! egui front end.

pub mod app;
pub mod dialogs;
pub mod error;
pub mod fonts;
pub mod keymap;
pub mod painter;
pub mod palette;
pub mod settings;
pub mod theme;
pub mod title;
pub mod widgets;
#[cfg(target_os = "linux")]
pub mod x11_dialog;
#[cfg(target_os = "linux")]
pub mod x11_sync;
#[cfg(target_os = "linux")]
pub mod x11_wm;
