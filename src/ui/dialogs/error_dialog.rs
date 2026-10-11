//! Error message box.

use eframe::egui;

use crate::ui::widgets::dialog::{button_row, Dialog};

/// Show `msg`. Returns true when dismissed.
pub fn error_dialog(ctx: &egui::Context, msg: &str) -> bool {
    let shown = Dialog::new("error", "Error").width(420.0).show(ctx, |ui| {
        ui.label(msg);
        ui.add_space(12.0);
        button_row(ui, |ui| ui.button("OK").clicked())
    });
    shown.close || shown.inner
}
