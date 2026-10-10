//! egui front end.

pub mod app;
pub mod dialogs;
pub mod error;
pub mod keymap;
pub mod painter;
pub mod palette;
pub mod settings;
pub mod title;
pub mod widgets;
#[cfg(target_os = "linux")]
pub mod x11_sync;
