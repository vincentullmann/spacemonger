//! Main window (port of AppController + FolderView): owns the app state and wires the panels,
//! dialogs and keyboard to the core.

mod commands;
mod treemap;
mod view;

use crate::core::camera::Camera;
use crate::core::fs::{drive_for_path, ScanJob};
use crate::core::geometry::Rect;
use crate::core::layout::{Item, LayoutParams, Reshape};
use crate::core::model::{Entry, EntryRef, Tree};
use crate::core::selection::{Marquee, Selection};
use crate::ui::dialogs::DriveDialog;
use crate::ui::error::AppError;
use crate::ui::keymap::action_for_keys;
use crate::ui::palette::Palette;
use crate::ui::settings::Settings;
use crate::ui::title::window_title;
use crate::ui::widgets::{path_bar, toolbar, CommandState};
use eframe::egui::{self, FontId, Vec2};
use std::path::PathBuf;
use std::time::Instant;


pub struct SpaceMonger {
    tree: Option<Tree>,
    drive: Option<crate::core::fs::Drive>,
    /// Index path of the deepest folder covering the whole view (derived from the camera).
    zoom: Vec<usize>,
    camera: Camera,

    /// This frame's layout, and what it was built from.
    items: Vec<Item>,
    layout_key: Option<(f32, f32, u64, Rect, Vec<Reshape>)>,
    /// Bumped whenever the tree or layout settings change.
    generation: u64,

    selection: Selection,
    marquee: Option<Marquee>,
    hovered: Option<usize>,
    hover_since: Instant,

    show_free: bool,
    dark: bool,
    applied_dark: Option<bool>,
    params: LayoutParams,

    dialog: Option<DriveDialog>,
    scan: Option<ScanJob>,
    /// Entries waiting for the delete confirmation, with their paths.
    confirm_delete: Option<(Vec<EntryRef>, Vec<PathBuf>)>,
    error: Option<AppError>,
    title: String,
    font: FontId,
}

impl SpaceMonger {
    pub fn new(cc: &eframe::CreationContext<'_>, open_path: Option<PathBuf>) -> Self {
        let settings = Settings::load(cc.storage);
        let mut app = Self {
            tree: None,
            drive: None,
            zoom: Vec::new(),
            camera: Camera::default(),
            items: Vec::new(),
            layout_key: None,
            generation: 0,
            selection: Selection::default(),
            marquee: None,
            hovered: None,
            hover_since: Instant::now(),
            show_free: settings.show_free,
            dark: settings.dark,
            applied_dark: None,
            params: LayoutParams::default(),
            dialog: None,
            scan: None,
            confirm_delete: None,
            error: None,
            title: String::new(),
            font: FontId::proportional(10.0),
        };
        match open_path {
            Some(p) => match drive_for_path(&p) {
                Ok(d) => app.start_scan(d, false, &cc.egui_ctx),
                Err(e) => app.error = Some(e.into()),
            },
            None => app.open_dialog(),
        }
        app
    }

    fn palette(&self) -> Palette {
        Palette::new(self.dark)
    }

    /// Force a layout rebuild (the tree or layout settings changed).
    fn invalidate(&mut self) {
        self.generation += 1;
    }

    fn params(&self) -> LayoutParams {
        LayoutParams { show_free: self.show_free, ..self.params }
    }

    fn selected_entry(&self) -> Option<&Entry> {
        self.tree.as_ref()?.entry(self.selection.primary()?)
    }

    /// Index into `items` of the primary selection, if it's laid out.
    fn selected_item(&self) -> Option<usize> {
        let r = self.selection.primary()?;
        self.items.iter().position(|it| it.index == Some(r.index) && it.folder == r.folder)
    }

    fn item_ref(&self, idx: usize) -> Option<EntryRef> {
        let it = &self.items[idx];
        Some(EntryRef::new(it.folder.clone(), it.index?))
    }

    fn command_state(&self) -> CommandState {
        CommandState {
            has_tree: self.tree.is_some(),
            zoomed: self.camera.zoomed(),
            sel_folder: self.selected_entry().is_some_and(|e| e.child().is_some()),
            has_sel: !self.selection.is_empty(),
            show_free: self.show_free,
            hidden: self.tree.as_ref().map_or(0, |t| t.hidden_count),
            dark: self.dark,
        }
    }

    fn modal_open(&self) -> bool {
        self.dialog.is_some() || self.scan.is_some() || self.error.is_some() || self.confirm_delete.is_some()
    }
}

impl eframe::App for SpaceMonger {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(target_os = "linux")]
        crate::ui::x11_sync::install(frame, &ctx, self.palette().background);
        #[cfg(not(target_os = "linux"))]
        let _ = frame;
        if self.applied_dark != Some(self.dark) {
            self.applied_dark = Some(self.dark);
            ctx.set_visuals(if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            #[cfg(target_os = "linux")]
            crate::ui::x11_sync::set_background(self.palette().background);
        }
        self.poll_scan();

        let st = self.command_state();
        let mut act = egui::Panel::top("toolbar").show(ui, |ui| toolbar(ui, &st)).inner;
        let pal = self.palette();
        let tm = egui::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(pal.background))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let target = self.tree.as_ref().and_then(|t| path_bar(ui, t, &self.zoom, pal, &self.font));
                if let Some(target) = target {
                    self.zoom_to(&target);
                }
                self.treemap(ui)
            })
            .inner;
        act = act.or(tm);

        if !self.modal_open() {
            act = act.or(ctx.input(action_for_keys));
        }
        if let Some(a) = act {
            self.run(a, &ctx);
        }

        self.dialogs(&ctx);

        let title = window_title(self.tree.as_ref(), &self.selection, &self.zoom);
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        // This frame is drawn at the window's current size: let the WM move on once it's shown.
        #[cfg(target_os = "linux")]
        crate::ui::x11_sync::frame_drawn(&ctx);
    }

    /// Runs once before every [`Self::ui`]. eframe 0.36 has no `update`; this is that hook.
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The previous frame is on screen now; release the window manager's resize step.
        #[cfg(target_os = "linux")]
        crate::ui::x11_sync::acknowledge();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette().background.to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        Settings { dark: self.dark, show_free: self.show_free }.save(storage);
    }
}
