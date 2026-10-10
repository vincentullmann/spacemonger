//! Error message box.

use eframe::egui::{self, Id};

/// Show `msg`. Returns true when dismissed.
pub fn error_dialog(ctx: &egui::Context, msg: &str) -> bool {
    let mut close = false;
    egui::Modal::new(Id::new("error")).show(ctx, |ui| {
        ui.set_max_width(460.0);
        ui.heading("Error");
        ui.label(msg);
        if ui.button("OK").clicked() {
            close = true;
        }
    });
    close
}
