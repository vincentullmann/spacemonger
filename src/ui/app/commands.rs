//! Running actions, scanning, deleting and the dialogs.

use super::SpaceMonger;
use crate::core::actions::Action;
use crate::core::fs::{Drive, ScanJob, ScanStatus};
use crate::core::model::EntryRef;
use crate::ui::dialogs::{confirm_delete, error_dialog, scan_dialog, DriveDialog};
use crate::ui::error::AppError;
use eframe::egui;
use std::path::PathBuf;

impl SpaceMonger {
    pub(super) fn open_dialog(&mut self) {
        self.dialog = Some(DriveDialog::with_volumes());
    }

    /// Scan `drive` (a whole volume if `is_drive`, otherwise a folder), dropping the current tree.
    pub(super) fn start_scan(&mut self, drive: Drive, is_drive: bool, ctx: &egui::Context) {
        if !is_drive {
            self.show_free = false;
        }
        if let Some(job) = self.scan.take() {
            job.cancel();
        }
        self.tree = None;
        self.live = None;
        self.zoom.clear();
        self.camera.reset();
        self.selection.clear();
        self.marquee = None;
        self.invalidate();

        let ctx = ctx.clone();
        self.scan = Some(ScanJob::spawn(drive.clone(), move || ctx.request_repaint()));
        self.drive = Some(drive);
    }

    pub(super) fn poll_scan(&mut self) {
        let Some(job) = &mut self.scan else { return };
        match job.poll() {
            ScanStatus::Running => {
                if let Some(t) = job.take_snapshot() {
                    self.live = Some(t);
                    self.invalidate();
                }
            }
            ScanStatus::Finished(tree) => {
                self.tree = tree;
                self.live = None;
                self.scan = None;
                self.invalidate();
            }
        }
    }

    pub(super) fn run(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Open => self.open_dialog(),
            Action::Reload => {
                if let Some(d) = self.drive.clone() {
                    let show_free = self.show_free;
                    self.start_scan(d, true, ctx);
                    self.show_free = show_free;
                }
            }
            Action::ZoomFull => self.zoom_to(&[]),
            Action::ZoomOut => self.zoom_out(),
            Action::ZoomIn => {
                if let Some(idx) = self.selected_item() {
                    self.zoom_in_item(idx);
                }
            }
            Action::ToggleFree => {
                self.show_free = !self.show_free;
                self.invalidate();
            }
            Action::RunOpen => self.open_selection(),
            Action::Delete => {
                let Some(t) = &self.tree else { return };
                let refs = self.selection.roots();
                if !refs.is_empty() {
                    let paths = refs.iter().map(|r| t.path_of(r)).collect();
                    self.confirm_delete = Some((refs, paths));
                }
            }
            Action::Hide => {
                let roots = self.selection.roots();
                if let (false, Some(t)) = (roots.is_empty(), &mut self.tree) {
                    for r in &roots {
                        t.hide(&r.folder, r.index);
                    }
                    self.selection.clear();
                    self.invalidate();
                }
            }
            Action::UnhideAll => {
                if let Some(t) = &mut self.tree {
                    t.unhide_all();
                    self.invalidate();
                }
            }
            Action::ToggleDark => self.dark = !self.dark,
            Action::ClearSelection => self.selection.clear(),
            Action::Frame => self.frame_selection(),
            Action::Nav(n) => self.navigate(n),
        }
    }

    /// Run or open every selected entry with the system's default handler.
    fn open_selection(&mut self) {
        let Some(t) = &self.tree else { return };
        let mut roots = self.selection.roots();
        roots.reverse();
        for r in roots {
            let p = t.path_of(&r);
            if let Err(source) = open::that_detached(&p) {
                self.error = Some(AppError::Open { path: p, source });
                break;
            }
        }
    }

    /// Move to trash. `refs` come from `Selection::roots`, so removing them in order never
    /// shifts a later one.
    fn delete_confirmed(&mut self, refs: Vec<EntryRef>, paths: Vec<PathBuf>) {
        let mut failed = Vec::new();
        for (r, path) in refs.into_iter().zip(paths) {
            match trash::delete(&path) {
                Ok(()) => {
                    if let Some(t) = &mut self.tree {
                        t.remove(&r.folder, r.index);
                    }
                }
                Err(e) => failed.push((path, e)),
            }
        }
        self.selection.clear();
        self.invalidate();
        if !failed.is_empty() {
            self.error = Some(AppError::Trash(failed));
        }
    }

    /// Show whichever dialogs are open and act on their answers.
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(dlg) = &mut self.dialog {
            let out = dlg.show(ctx);
            if let Some(e) = out.error {
                self.error = Some(e.into());
            }
            if let Some((d, is_drive)) = out.chosen {
                self.dialog = None;
                self.start_scan(d, is_drive, ctx);
            } else if out.closed {
                self.dialog = None;
            }
        }

        if let Some(job) = &self.scan {
            if scan_dialog(ctx, job) {
                job.cancel();
                self.scan = None;
                self.live = None;
                self.invalidate();
            }
        }

        if let Some((_, paths)) = &self.confirm_delete {
            if let Some(yes) = confirm_delete(ctx, paths) {
                let (refs, paths) = self.confirm_delete.take().unwrap_or_default();
                if yes {
                    self.delete_confirmed(refs, paths);
                }
            }
        }
        if let Some(msg) = &self.error {
            if error_dialog(ctx, &msg.to_string()) {
                self.error = None;
            }
        }
    }
}
