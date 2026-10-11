//! "Open": type or browse to a folder, or click a mounted volume to scan it.
//!
//! Laid out top to bottom: the folder line, the drives table, and (later) a list of recently
//! scanned paths under it, built like the drives table.

use crate::core::fs::{drive_for_path, volumes, Drive, FsError};
use crate::ui::widgets::dialog::Dialog;
use crate::utils::format;
use eframe::egui::{self, Align, Layout, RichText, Sense, Shape, Ui, Vec2};
use egui_phosphor::regular as icon;
use std::path::PathBuf;

/// Width of the dialog's contents.
const WIDTH: f32 = 520.0;
const ROW_H: f32 = 30.0;
const ICON_W: f32 = 26.0;
const BAR_W: f32 = 90.0;
const BAR_H: f32 = 6.0;
const SIZE_W: f32 = 130.0;
const FS_W: f32 = 76.0;

pub struct DriveDialog {
    drives: Vec<Drive>,
    path: String,
    /// Put the cursor in the folder field on the first frame.
    focus: bool,
}

/// What happened in the dialog this frame.
#[derive(Default)]
pub struct DriveDialogOutcome {
    /// Something to scan, and whether it's a whole volume (rather than a folder).
    pub chosen: Option<(Drive, bool)>,
    /// Closed without choosing.
    pub closed: bool,
    pub error: Option<FsError>,
}

impl DriveDialog {
    /// A dialog listing the currently mounted volumes.
    pub fn with_volumes() -> Self {
        Self {
            drives: volumes(),
            path: String::new(),
            focus: true,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> DriveDialogOutcome {
        let mut out = DriveDialogOutcome::default();
        let shown = Dialog::new("open", "Open").width(WIDTH).show(ctx, |ui| {
            self.folder_line(ui, &mut out);
            ui.add_space(14.0);
            heading(ui, "Drives");
            if self.drives.is_empty() {
                ui.weak("No drives found.");
            }
            egui::ScrollArea::vertical()
                .max_height(8.0 * ROW_H)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for d in &self.drives {
                        if drive_row(ui, d).clicked() {
                            out.chosen = Some((d.clone(), true));
                        }
                    }
                });
        });
        out.closed |= shown.close;
        out
    }

    /// "Folder: [path] [browse]". Enter scans the typed path; a folder picked in the file
    /// browser is scanned straight away.
    fn folder_line(&mut self, ui: &mut Ui, out: &mut DriveDialogOutcome) {
        ui.horizontal(|ui| {
            ui.label("Folder");
            let browse_w = ui.spacing().interact_size.y + 6.0;
            let w = ui.available_width() - browse_w - ui.spacing().item_spacing.x;
            let te = ui.add(
                egui::TextEdit::singleline(&mut self.path)
                    .hint_text("Path to a folder, Enter to scan")
                    .desired_width(w),
            );
            if std::mem::take(&mut self.focus) {
                te.request_focus();
            }
            let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let browse = ui
                .add(
                    egui::Button::new(RichText::new(icon::FOLDER_OPEN).size(15.0))
                        .min_size(Vec2::splat(browse_w)),
                )
                .on_hover_text("Browse…");
            if browse.clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Select Folder")
                    .pick_folder()
                {
                    self.path = p.display().to_string();
                    self.open_path(out);
                }
            }
            if enter && !self.path.trim().is_empty() {
                self.open_path(out);
            }
        });
    }

    fn open_path(&self, out: &mut DriveDialogOutcome) {
        match drive_for_path(&PathBuf::from(self.path.trim())) {
            Ok(d) => out.chosen = Some((d, false)),
            Err(e) => out.error = Some(e),
        }
    }
}

/// A small section title over a list.
fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().weak());
    ui.add_space(2.0);
}

/// One volume: icon, mount point, how full it is (bar, used / total) and file system. The
/// whole row is one button.
fn drive_row(ui: &mut Ui, d: &Drive) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
    // Behind the contents, filled in once we know whether it's hovered.
    let bg = ui.painter().add(Shape::Noop);
    let used = d.total.saturating_sub(d.free);
    let frac = if d.total > 0 {
        used as f32 / d.total as f32
    } else {
        0.0
    };

    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(6.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    let fixed = |ui: &mut Ui, w: f32, add: &mut dyn FnMut(&mut Ui)| {
        ui.allocate_ui_with_layout(
            Vec2::new(w, ROW_H),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_min_size(Vec2::new(w, ROW_H));
                add(ui)
            },
        );
    };
    row.add_sized(
        Vec2::new(ICON_W, ROW_H),
        egui::Label::new(RichText::new(drive_icon(&d.fs)).size(16.0)).selectable(false),
    );
    let name_w = row.available_width() - BAR_W - SIZE_W - FS_W - 3.0 * row.spacing().item_spacing.x;
    row.allocate_ui_with_layout(
        Vec2::new(name_w, ROW_H),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(name_w, ROW_H));
            ui.add(egui::Label::new(&d.name).selectable(false).truncate());
        },
    );
    fixed(&mut row, BAR_W, &mut |ui| {
        ui.add(
            egui::ProgressBar::new(frac)
                .desired_width(BAR_W)
                .desired_height(BAR_H),
        );
    });
    fixed(&mut row, SIZE_W, &mut |ui| {
        ui.add(
            egui::Label::new(format!(
                "{} / {}",
                format::size_string(used, 0, false),
                format::size_string(d.total, 0, false)
            ))
            .selectable(false),
        );
    });
    fixed(&mut row, FS_W, &mut |ui| {
        ui.add(
            egui::Label::new(RichText::new(&d.fs).weak())
                .selectable(false)
                .truncate(),
        );
    });

    let visuals = ui.style().interact(&resp);
    if resp.hovered() || resp.has_focus() {
        ui.painter().set(
            bg,
            Shape::rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill),
        );
    }
    resp
}

/// A network share or a local disk.
fn drive_icon(fs: &str) -> &'static str {
    const NETWORK: [&str; 7] = ["nfs", "nfs4", "cifs", "smb3", "smbfs", "fuse.sshfs", "9p"];
    if NETWORK.contains(&fs) {
        icon::NETWORK
    } else {
        icon::HARD_DRIVE
    }
}
