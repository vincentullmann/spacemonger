//! Progress while a scan runs, floating over the treemap as it fills in. Not modal: it can be
//! dragged out of the way.

use super::confirm_delete::buttons;
use crate::core::fs::ScanJob;
use crate::ui::theme::accent;
use crate::ui::widgets::dialog::Dialog;
use crate::utils::text::elide_start;
use eframe::egui::{self, Align2, Vec2};
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Show scan progress, with Pause / Resume and Cancel. Returns true when the user cancels.
pub fn scan_dialog(ctx: &egui::Context, job: &ScanJob) -> bool {
    let ctl = &job.ctl;
    let used = job.drive.total.saturating_sub(job.drive.free);
    let bytes = ctl.bytes.load(Ordering::Relaxed);
    let paused = ctl.is_paused();
    let title = if paused { "Scan paused" } else { "Scanning…" };
    let shown = Dialog::new("scan", title)
        .modal(false)
        .closable(false)
        .anchor(Align2::RIGHT_BOTTOM, Vec2::splat(12.0))
        .width(360.0)
        .show(ctx, |ui| {
            let cur = ctl.current.lock().map(|s| s.clone()).unwrap_or_default();
            ui.add(egui::Label::new(elide_start(&cur, 55)).selectable(false));
            ui.add_space(4.0);
            egui::Grid::new("scan_counts")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.weak("Files");
                    ui.label(ctl.files.load(Ordering::Relaxed).to_string());
                    ui.end_row();
                    ui.weak("Folders");
                    ui.label(ctl.folders.load(Ordering::Relaxed).to_string());
                    ui.end_row();
                });
            ui.add_space(6.0);
            let frac = if used > 0 {
                (bytes as f32 / used as f32).min(1.0)
            } else {
                0.0
            };
            ui.add(
                egui::ProgressBar::new(frac)
                    .show_percentage()
                    .fill(accent(ui.visuals())),
            );
            ui.add_space(10.0);
            buttons(ui, |ui| {
                let cancel = ui.button("Cancel").clicked();
                if ui.button(if paused { "Resume" } else { "Pause" }).clicked() {
                    ctl.set_paused(!paused);
                }
                cancel
            })
        });
    let cancel = shown.inner;
    if !cancel && !paused {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    cancel
}
