//! Progress while a scan runs, floating over the treemap as it fills in.

use crate::core::fs::ScanJob;
use crate::utils::text::elide_start;
use eframe::egui::{self, Align2};
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Show scan progress. Returns true when the user cancels.
pub fn scan_dialog(ctx: &egui::Context, job: &ScanJob) -> bool {
    let ctl = &job.ctl;
    let used = job.drive.total.saturating_sub(job.drive.free);
    let bytes = ctl.bytes.load(Ordering::Relaxed);
    let mut cancel = false;
    egui::Window::new("Scanning Disk...")
        .anchor(Align2::RIGHT_BOTTOM, [-12.0, -12.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_width(360.0);
            let cur = ctl.current.lock().map(|s| s.clone()).unwrap_or_default();
            ui.label(elide_start(&cur, 55));
            ui.label(format!(
                "Files Found:  {}",
                ctl.files.load(Ordering::Relaxed)
            ));
            ui.label(format!(
                "Folders Found:  {}",
                ctl.folders.load(Ordering::Relaxed)
            ));
            let frac = if used > 0 {
                (bytes as f32 / used as f32).min(1.0)
            } else {
                0.0
            };
            ui.add(egui::ProgressBar::new(frac).show_percentage());
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
    if !cancel {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    cancel
}
