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
use crate::ui::fonts;
use crate::ui::palette::Palette;
use crate::ui::settings::{Settings, SettingsWindow};
use crate::ui::title::window_title;
use crate::ui::widgets::{path_bar, toolbar, CommandState};
use eframe::egui::{self, Vec2};
use std::path::PathBuf;
use std::time::Instant;

pub struct SpaceMonger {
    tree: Option<Tree>,
    /// Snapshot of a running scan, drawn (but not interactive) until the scan finishes.
    live: Option<Tree>,
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

    settings: Settings,
    settings_window: SettingsWindow,
    /// Settings as of the last frame, to react to what changed.
    applied: Option<Settings>,

    dialog: Option<DriveDialog>,
    scan: Option<ScanJob>,
    /// Entries waiting for the delete confirmation, with their paths.
    confirm_delete: Option<(Vec<EntryRef>, Vec<PathBuf>)>,
    error: Option<AppError>,
    title: String,
}

impl SpaceMonger {
    pub fn new(cc: &eframe::CreationContext<'_>, open_path: Option<PathBuf>) -> Self {
        let settings = Settings::load(cc.storage);
        let mut app = Self {
            tree: None,
            live: None,
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
            settings,
            settings_window: SettingsWindow::default(),
            applied: None,
            dialog: None,
            scan: None,
            confirm_delete: None,
            error: None,
            title: String::new(),
        };
        app.apply_settings(&cc.egui_ctx);
        match open_path {
            Some(p) => match drive_for_path(&p) {
                Ok(d) => app.start_scan(d, false, &cc.egui_ctx),
                Err(e) => app.error = Some(e.into()),
            },
            None => app.open_dialog(),
        }
        app
    }

    /// Dark colours in effect (the theme may follow the system).
    fn dark(&self, ctx: &egui::Context) -> bool {
        ctx.theme() == egui::Theme::Dark
    }

    fn palette(&self, ctx: &egui::Context) -> Palette {
        Palette::new(self.dark(ctx), &self.settings)
    }

    /// Push changed settings out to egui, the camera, the tree and the layout.
    fn apply_settings(&mut self, ctx: &egui::Context) {
        let s = &self.settings;
        let old = self.applied.as_ref();
        if old.is_none_or(|o| o.general.theme != s.general.theme) {
            ctx.set_theme(s.general.theme);
        }
        if old.is_none_or(|o| o.font.family != s.font.family) {
            fonts::apply(ctx, &s.font.family);
        }
        if old.is_none_or(|o| o.format_options() != s.format_options()) {
            crate::utils::format::set_options(s.format_options());
        }
        self.camera.params = s.camera_params();
        if let Some(t) = &mut self.tree {
            if t.dotfiles_hidden != s.scan.ignore_hidden {
                t.set_dotfiles_hidden(s.scan.ignore_hidden);
                self.selection.clear();
                self.generation += 1;
            }
        }
        if old.is_some_and(|o| o.layout_params(false) != s.layout_params(false)) {
            self.generation += 1;
        }
        self.applied = Some(self.settings.clone());
    }

    /// Force a layout rebuild (the tree or layout settings changed).
    fn invalidate(&mut self) {
        self.generation += 1;
    }

    fn params(&self) -> LayoutParams {
        let dots = self.shown_tree().is_some_and(|t| t.dotfiles_hidden);
        self.settings.layout_params(dots)
    }

    /// The tree to draw: the scanned one, or a running scan's snapshot.
    fn shown_tree(&self) -> Option<&Tree> {
        self.tree.as_ref().or(self.live.as_ref())
    }

    fn selected_entry(&self) -> Option<&Entry> {
        self.tree.as_ref()?.entry(self.selection.primary()?)
    }

    /// Index into `items` of the primary selection, if it's laid out.
    fn selected_item(&self) -> Option<usize> {
        let r = self.selection.primary()?;
        self.items
            .iter()
            .position(|it| it.index == Some(r.index) && it.folder == r.folder)
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
            show_free: self.settings.layout.show_free,
            hidden: self.tree.as_ref().map_or(0, |t| t.hidden_count),
        }
    }

    fn modal_open(&self) -> bool {
        self.dialog.is_some() || self.error.is_some() || self.confirm_delete.is_some()
    }
}

impl eframe::App for SpaceMonger {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(target_os = "linux")]
        {
            let bg = self.palette(&ctx).background;
            crate::ui::x11_sync::install(frame, &ctx, bg);
            crate::ui::x11_sync::set_background(bg);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = frame;
        self.poll_scan();

        let st = self.command_state();
        let mut act = egui::Panel::top("toolbar")
            .show(ui, |ui| toolbar(ui, &st))
            .inner;
        let pal = self.palette(&ctx);
        let (bar_font, bar_h) = (self.settings.bar_font(), self.settings.path_bar.height);
        let tm = egui::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(pal.background))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let target = self
                    .shown_tree()
                    .and_then(|t| path_bar(ui, t, &self.zoom, &pal, &bar_font, bar_h));
                if let Some(target) = target.filter(|_| self.tree.is_some()) {
                    self.zoom_to(&target);
                }
                self.treemap(ui)
            })
            .inner;
        act = act.or(tm);

        if !self.modal_open() && !self.settings_window.capturing() && !ctx.text_edit_focused() {
            act = act.or(ctx.input(|i| self.settings.keys.action_for_keys(i)));
        }
        if let Some(a) = act {
            self.run(a, &ctx);
        }

        self.dialogs(&ctx);
        self.settings_window.show(&ctx, &mut self.settings);
        if self.applied.as_ref() != Some(&self.settings) {
            self.apply_settings(&ctx);
        }

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

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        Palette::new(visuals.dark_mode, &self.settings)
            .background
            .to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.settings.save(storage);
    }
}
