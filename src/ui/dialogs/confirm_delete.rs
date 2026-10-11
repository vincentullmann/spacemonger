//! "Move to trash?" confirmation.

use crate::ui::widgets::dialog::Dialog;
use eframe::egui::{self, Align, Layout};
use std::path::PathBuf;

/// Ask before moving `paths` to the trash. `Some(true)` = go ahead, `Some(false)` = cancel,
/// `None` = not answered yet.
pub fn confirm_delete(ctx: &egui::Context, paths: &[PathBuf]) -> Option<bool> {
    let shown = Dialog::new("confirm_delete", "Move to Trash")
        .width(440.0)
        .show(ctx, |ui| {
            if let [path] = paths {
                ui.label("Move this to the trash?");
                ui.add_space(6.0);
                ui.label(path.display().to_string());
            } else {
                ui.label(format!("Move these {} items to the trash?", paths.len()));
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for path in paths.iter().rev() {
                            ui.label(path.display().to_string());
                        }
                    });
            }
            ui.add_space(12.0);
            buttons(ui, |ui| {
                // Right to left: the default action sits at the far right.
                if ui.button("Move to Trash").clicked() {
                    Some(true)
                } else if ui.button("Cancel").clicked() {
                    Some(false)
                } else {
                    None
                }
            })
        });
    if shown.close {
        Some(false)
    } else {
        shown.inner
    }
}

/// A row of buttons along the bottom right of a dialog, laid out right to left.
pub(super) fn buttons<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), add)
        .inner
}
