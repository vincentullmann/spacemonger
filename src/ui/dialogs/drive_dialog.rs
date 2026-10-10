//! "Select Drive to View": pick a mounted volume or type / browse to a folder.

use crate::core::fs::{drive_for_path, volumes, Drive};
use crate::utils::format;
use eframe::egui::{self, Id};
use std::path::PathBuf;

pub struct DriveDialog {
    drives: Vec<Drive>,
    selected: Option<usize>,
    path: String,
}

/// What happened in the dialog this frame.
#[derive(Default)]
pub struct DriveDialogOutcome {
    /// Something to scan, and whether it's a whole volume (rather than a folder).
    pub chosen: Option<(Drive, bool)>,
    /// Closed without choosing.
    pub closed: bool,
    pub error: Option<String>,
}

impl DriveDialog {
    /// A dialog listing the currently mounted volumes.
    pub fn with_volumes() -> Self {
        Self { drives: volumes(), selected: None, path: String::new() }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> DriveDialogOutcome {
        let mut out = DriveDialogOutcome::default();
        let modal = egui::Modal::new(Id::new("drive_dialog")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("Select Drive to View");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                if self.drives.is_empty() {
                    ui.label("No drives found.");
                }
                for (i, d) in self.drives.iter().enumerate() {
                    let used = d.total.saturating_sub(d.free);
                    let text = format!(
                        "{}    {} / {}  ({})    {}",
                        d.name,
                        format::size_string(used, 0, false),
                        format::size_string(d.total, 0, false),
                        format::size_string(used, d.total, true),
                        d.fs
                    );
                    let r = ui.add(egui::Button::selectable(self.selected == Some(i), text));
                    if r.clicked() {
                        self.selected = Some(i);
                    }
                    if r.double_clicked() {
                        out.chosen = Some((d.clone(), true));
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Folder:");
                let te = ui.add(egui::TextEdit::singleline(&mut self.path).desired_width(240.0));
                let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Browse...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Select Folder").pick_folder() {
                        self.path = p.display().to_string();
                    }
                }
                if (ui.add_enabled(!self.path.is_empty(), egui::Button::new("Open Folder")).clicked() || enter)
                    && !self.path.is_empty()
                {
                    match drive_for_path(&PathBuf::from(self.path.trim())) {
                        Some(d) => out.chosen = Some((d, false)),
                        None => out.error = Some(format!("Not a readable folder: {}", self.path)),
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(self.selected.is_some(), egui::Button::new("OK")).clicked() {
                    if let Some(d) = self.selected.and_then(|i| self.drives.get(i)) {
                        out.chosen = Some((d.clone(), true));
                    }
                }
                if ui.button("Cancel").clicked() {
                    out.closed = true;
                }
            });
        });
        if modal.should_close() {
            out.closed = true;
        }
        out
    }
}
