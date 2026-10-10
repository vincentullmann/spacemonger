//! "Move to trash?" confirmation.

use eframe::egui::{self, Id};
use std::path::PathBuf;

/// Ask before moving `paths` to the trash. `Some(true)` = go ahead, `Some(false)` = cancel,
/// `None` = not answered yet.
pub fn confirm_delete(ctx: &egui::Context, paths: &[PathBuf]) -> Option<bool> {
    let mut answer = None;
    egui::Modal::new(Id::new("confirm_delete")).show(ctx, |ui| {
        ui.set_max_width(460.0);
        ui.heading("Delete");
        if let [path] = paths {
            ui.label(format!("Move to trash?\n\n{}", path.display()));
        } else {
            ui.label(format!("Move {} items to trash?", paths.len()));
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for path in paths.iter().rev() {
                        ui.label(path.display().to_string());
                    }
                });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Move to Trash").clicked() {
                answer = Some(true);
            }
            if ui.button("Cancel").clicked() {
                answer = Some(false);
            }
        });
    });
    answer
}
