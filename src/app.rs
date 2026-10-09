//! Main window: toolbar, treemap view, dialogs (port of AppController + FolderView).

use crate::colors::Palette;
use crate::format;
use crate::layout::{self, Item, Params};
use crate::scan::{self, Drive, ScanControl, Tree};
use eframe::egui::{
    self, Align2, Color32, FontId, Id, Order, Painter, Pos2, Rect, Sense, Stroke, Vec2,
};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

const APP_NAME: &str = "SpaceMonger One";
const INFOTIP_DELAY: Duration = Duration::from_millis(250);
const ANIM_DURATION: f32 = 0.18; // seconds

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Open,
    Reload,
    ZoomFull,
    ZoomIn,
    ZoomOut,
    ToggleFree,
    RunOpen,
    Delete,
    Hide,
    ToggleDark,
}

struct ScanJob {
    drive: Drive,
    ctl: Arc<ScanControl>,
    rx: mpsc::Receiver<Option<Tree>>,
}

struct DriveDialog {
    drives: Vec<Drive>,
    selected: Option<usize>,
    path: String,
}

/// Zoom animation: a box morphing between two rectangles (view-local coordinates).
struct Anim {
    from: Rect,
    to: Rect,
    start: Instant,
    target: Vec<usize>,
}

/// A selected entry, identified by its folder path and index.
type Sel = (Rc<[usize]>, usize);

pub struct SpaceMonger {
    tree: Option<Tree>,
    drive: Option<Drive>,
    /// Index path of the folder currently filling the view.
    zoom: Vec<usize>,

    items: Vec<Item>,
    layout_key: Option<(i32, i32, u64)>,
    generation: u64,

    selected: Option<Sel>,
    hovered: Option<usize>,
    hover_since: Instant,
    /// Accumulated wheel delta, so trackpads don't zoom on every tiny scroll.
    scroll_accum: f32,
    anim: Option<Anim>,

    show_free: bool,
    dark: bool,
    applied_dark: Option<bool>,
    params: Params,

    dialog: Option<DriveDialog>,
    scan: Option<ScanJob>,
    confirm_delete: Option<(Sel, PathBuf)>,
    error: Option<String>,
    title: String,
    font: FontId,
}

impl SpaceMonger {
    pub fn new(cc: &eframe::CreationContext<'_>, open_path: Option<PathBuf>) -> Self {
        let get = |k: &str| cc.storage.and_then(|s| s.get_string(k));
        let mut app = Self {
            tree: None,
            drive: None,
            zoom: Vec::new(),
            items: Vec::new(),
            layout_key: None,
            generation: 0,
            selected: None,
            hovered: None,
            hover_since: Instant::now(),
            scroll_accum: 0.0,
            anim: None,
            show_free: get("show_free").is_none_or(|v| v == "true"),
            dark: get("dark").is_some_and(|v| v == "true"),
            applied_dark: None,
            params: Params::default(),
            dialog: None,
            scan: None,
            confirm_delete: None,
            error: None,
            title: String::new(),
            font: FontId::proportional(10.0),
        };
        match open_path {
            Some(p) => match scan::drive_for_path(&p) {
                Some(d) => app.start_scan(d, false, &cc.egui_ctx),
                None => app.error = Some(format!("Cannot open {}", p.display())),
            },
            None => app.open_dialog(),
        }
        app
    }

    fn palette(&self) -> Palette {
        Palette::new(self.dark)
    }

    fn invalidate(&mut self) {
        self.generation += 1;
    }

    // -- selection helpers ---------------------------------------------------

    fn selected_entry(&self) -> Option<&scan::Entry> {
        let (f, i) = self.selected.as_ref()?;
        self.tree.as_ref()?.entry_at(f, *i)
    }

    fn selected_path(&self) -> Option<PathBuf> {
        let (f, i) = self.selected.as_ref()?;
        Some(self.tree.as_ref()?.full_path(f, Some(*i)))
    }

    fn selected_item(&self) -> Option<usize> {
        let (f, i) = self.selected.as_ref()?;
        self.items.iter().position(|it| it.index == Some(*i) && it.folder == *f)
    }

    fn item_sel(&self, idx: usize) -> Sel {
        let it = &self.items[idx];
        (it.folder.clone(), it.index.unwrap_or(0))
    }

    fn is_selected(&self, it: &Item) -> bool {
        matches!(&self.selected, Some((f, i)) if it.index == Some(*i) && it.folder == *f)
    }

    // -- commands ------------------------------------------------------------

    fn open_dialog(&mut self) {
        self.dialog = Some(DriveDialog { drives: scan::volumes(), selected: None, path: String::new() });
    }

    fn start_scan(&mut self, drive: Drive, is_drive: bool, ctx: &egui::Context) {
        if !is_drive {
            self.show_free = false;
        }
        if let Some(job) = self.scan.take() {
            job.ctl.cancel();
        }
        self.tree = None;
        self.zoom.clear();
        self.selected = None;
        self.anim = None;
        self.invalidate();

        let ctl = Arc::new(ScanControl::default());
        let (tx, rx) = mpsc::channel();
        let (d, c, ctx) = (drive.clone(), ctl.clone(), ctx.clone());
        std::thread::spawn(move || {
            let res = scan::scan(&d, &c);
            let _ = tx.send(res);
            ctx.request_repaint();
        });
        self.drive = Some(drive);
        self.scan = Some(ScanJob { drive: self.drive.clone().unwrap(), ctl, rx });
    }

    fn poll_scan(&mut self) {
        let Some(job) = &self.scan else { return };
        match job.rx.try_recv() {
            Ok(Some(tree)) => {
                self.tree = Some(tree);
                self.scan = None;
                self.invalidate();
            }
            Ok(None) | Err(mpsc::TryRecvError::Disconnected) => self.scan = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn set_zoom(&mut self, target: Vec<usize>, from: Rect, to: Rect) {
        if target == self.zoom {
            return;
        }
        self.anim = Some(Anim { from, to, start: Instant::now(), target });
    }

    fn finish_anim(&mut self) {
        if let Some(a) = self.anim.take() {
            self.zoom = a.target;
            self.selected = None;
            self.invalidate();
        }
    }

    fn view_size(&self) -> (i32, i32) {
        self.layout_key.map_or((0, 0), |(w, h, _)| (w, h))
    }

    fn zoom_in_item(&mut self, idx: usize) {
        let it = &self.items[idx];
        let Some(i) = it.index else { return };
        if !it.is_folder {
            return;
        }
        let mut target = it.folder.to_vec();
        target.push(i);
        let from = irect(it.x, it.y, it.w, it.h);
        let (w, h) = self.view_size();
        self.set_zoom(target, from, irect(0, 0, w, h));
    }

    /// Zoom one level deeper, towards the folder under the point.
    fn zoom_towards(&mut self, x: i32, y: i32) {
        // Deepest item containing the point (children come after parents).
        let Some(it) = self.items.iter().rev().find(|it| !it.is_free && it.contains(x, y)) else { return };
        let mut path = it.folder.to_vec();
        if it.is_folder {
            if let Some(i) = it.index {
                path.push(i);
            }
        }
        let depth = self.zoom.len();
        if path.len() <= depth {
            return;
        }
        let (parent, index) = (&path[..depth], path[depth]);
        let Some(idx) = self.items.iter().position(|it| it.index == Some(index) && *it.folder == *parent) else { return };
        self.zoom_in_item(idx);
    }

    fn zoom_out_to(&mut self, target: Vec<usize>) {
        let (w, h) = self.view_size();
        let to = Rect::from_center_size(Pos2::new(w as f32 / 2.0, h as f32 / 2.0), Vec2::new(w as f32, h as f32) * 0.15);
        self.set_zoom(target, irect(0, 0, w, h), to);
    }

    fn run(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Open => self.open_dialog(),
            Action::Reload => {
                if let Some(d) = self.drive.clone() {
                    let show_free = self.show_free;
                    self.start_scan(d, true, ctx);
                    self.show_free = show_free;
                }
            }
            Action::ZoomFull => self.zoom_out_to(Vec::new()),
            Action::ZoomOut => {
                if !self.zoom.is_empty() {
                    let t = self.zoom[..self.zoom.len() - 1].to_vec();
                    self.zoom_out_to(t);
                }
            }
            Action::ZoomIn => {
                if let Some(idx) = self.selected_item() {
                    self.zoom_in_item(idx);
                }
            }
            Action::ToggleFree => {
                self.show_free = !self.show_free;
                self.invalidate();
            }
            Action::RunOpen => {
                if let Some(p) = self.selected_path() {
                    if let Err(e) = open::that_detached(&p) {
                        self.error = Some(format!("Cannot open {}:\n{e}", p.display()));
                    }
                }
            }
            Action::Delete => {
                if let (Some(sel), Some(p)) = (self.selected.clone(), self.selected_path()) {
                    self.confirm_delete = Some((sel, p));
                }
            }
            Action::Hide => {
                if let (Some(sel), Some(t)) = (self.selected.take(), &mut self.tree) {
                    t.remove(&sel.0, sel.1);
                    self.invalidate();
                }
            }
            Action::ToggleDark => self.dark = !self.dark,
        }
    }

    fn delete_confirmed(&mut self, sel: Sel, path: PathBuf) {
        match trash::delete(&path) {
            Ok(()) => {
                if let Some(t) = &mut self.tree {
                    t.remove(&sel.0, sel.1);
                }
                self.selected = None;
                self.invalidate();
            }
            Err(e) => self.error = Some(format!("Failed to move {} to trash:\n{e}", path.display())),
        }
    }

    // -- title ---------------------------------------------------------------

    fn compute_title(&self) -> String {
        let Some(t) = &self.tree else { return APP_NAME.to_string() };
        if let (Some(e), Some(p)) = (self.selected_entry(), self.selected_path()) {
            return format!(
                "{}  -  {}  -  {}  -  {APP_NAME}",
                p.display(),
                format::size_string(e.size, t.total_space, true),
                format::size_string(e.size, t.total_space, false),
            );
        }
        let size = if self.zoom.is_empty() {
            t.total_space
        } else {
            t.folder_at(&self.zoom).map_or(0, |f| f.total)
        };
        format!(
            "{}  -  {} Total  -  {} Free  -  {APP_NAME}",
            t.full_path(&self.zoom, None).display(),
            format::size_string(size, t.total_space, false),
            format::size_string(t.free_space, t.free_space, false),
        )
    }

    // -- toolbar -------------------------------------------------------------

    fn toolbar(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let has_tree = self.tree.is_some();
        let zoomed = !self.zoom.is_empty();
        let sel_folder = self.selected_entry().is_some_and(|e| e.child().is_some());
        let has_sel = self.selected.is_some();
        let show_free = self.show_free;
        let dark = self.dark;
        ui.horizontal(|ui| {
            let mut act = None;
            let mut btn = |ui: &mut egui::Ui, enabled: bool, b: egui::Button, a: Action| {
                if ui.add_enabled(enabled, b).clicked() {
                    act = Some(a);
                }
            };
            let b = egui::Button::new;
            btn(ui, true, b("📂 Open"), Action::Open);
            btn(ui, has_tree, b("⟳ Reload"), Action::Reload);
            ui.separator();
            btn(ui, has_tree && zoomed, b("⏏ Zoom Full"), Action::ZoomFull);
            btn(ui, has_tree && sel_folder, b("➕ Zoom In"), Action::ZoomIn);
            btn(ui, has_tree && zoomed, b("➖ Zoom Out"), Action::ZoomOut);
            ui.separator();
            btn(ui, has_tree, b("Free Space").selected(show_free), Action::ToggleFree);
            ui.separator();
            btn(ui, has_sel, b("▶ Run or Open"), Action::RunOpen);
            btn(ui, has_sel, b("🗑 Delete"), Action::Delete);
            btn(ui, has_sel, b("Hide"), Action::Hide);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                btn(ui, true, b(if dark { "☀" } else { "🌙" }), Action::ToggleDark);
            });
            act
        })
        .inner
    }

    // -- treemap -------------------------------------------------------------

    fn treemap(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
        let ppp = ui.ctx().pixels_per_point();
        let origin = Pos2::new((resp.rect.min.x * ppp).round() / ppp, (resp.rect.min.y * ppp).round() / ppp);
        let (w, h) = (resp.rect.width() as i32, resp.rect.height() as i32);

        // Rebuild layout on resize / data change.
        let key = (w, h, self.generation);
        if self.layout_key != Some(key) {
            self.layout_key = Some(key);
            self.items = match &self.tree {
                Some(t) => match t.folder_at(&self.zoom) {
                    Some(f) => {
                        let p = Params { show_free: self.show_free, ..self.params };
                        layout::build(f, self.zoom.clone(), w, h, p)
                    }
                    None => Vec::new(),
                },
                None => Vec::new(),
            };
            self.hovered = None;
        }

        let mut act = None;

        // Pointer / hover handling.
        let local = |p: Pos2| ((p.x - origin.x) as i32, (p.y - origin.y) as i32);
        let hit = resp
            .hover_pos()
            .and_then(|p| {
                let (x, y) = local(p);
                layout::hit_test(&self.items, x, y)
            })
            .filter(|_| self.anim.is_none());
        if hit != self.hovered {
            self.hovered = hit;
            self.hover_since = Instant::now();
        }

        let pal = self.palette();
        let d = Draw { painter: &painter, origin, ppp, pal, font: self.font.clone() };

        d.fill(pal.background, 0, 0, w, h);
        if let Some(tree) = &self.tree {
            // Parents come before children, so a selected folder's children stay visible.
            for (i, it) in self.items.iter().enumerate() {
                d.item(tree, it, self.is_selected(it), self.hovered == Some(i));
            }
        }

        // Scroll wheel: up = zoom one level towards the pointer, down = zoom out.
        if resp.hovered() && self.anim.is_none() && self.tree.is_some() {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                self.scroll_accum += dy;
                if self.scroll_accum >= 30.0 {
                    self.scroll_accum = 0.0;
                    if let Some(p) = resp.hover_pos() {
                        let (x, y) = local(p);
                        self.zoom_towards(x, y);
                    }
                } else if self.scroll_accum <= -30.0 {
                    self.scroll_accum = 0.0;
                    act = act.or(Some(Action::ZoomOut));
                }
            }
        }

        let pointer_hit = || {
            resp.interact_pointer_pos().and_then(|p| {
                let (x, y) = local(p);
                layout::hit_test(&self.items, x, y)
            })
        };
        if resp.clicked() || resp.secondary_clicked() {
            self.selected = pointer_hit().map(|i| self.item_sel(i));
        }
        if resp.double_clicked() {
            if let Some(i) = pointer_hit() {
                self.selected = Some(self.item_sel(i));
                act = Some(if self.items[i].is_folder { Action::ZoomIn } else { Action::RunOpen });
            }
        }

        let has_sel = self.selected.is_some();
        let sel_folder = self.selected_entry().is_some_and(|e| e.child().is_some());
        let zoomed = !self.zoom.is_empty();
        let show_free = self.show_free;
        resp.context_menu(|ui| {
            let mut item = |ui: &mut egui::Ui, enabled: bool, label: &str, a: Action| {
                if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                    act = Some(a);
                    ui.close();
                }
            };
            item(ui, sel_folder, "Zoom In", Action::ZoomIn);
            item(ui, zoomed, "Zoom Out", Action::ZoomOut);
            item(ui, zoomed, "Zoom Full", Action::ZoomFull);
            ui.separator();
            item(ui, has_sel, "Run / Open", Action::RunOpen);
            item(ui, has_sel, "Delete", Action::Delete);
            item(ui, has_sel, "Hide (H)", Action::Hide);
            ui.separator();
            item(ui, true, "Open Drive...", Action::Open);
            item(ui, true, "Rescan Drive", Action::Reload);
            let label = if show_free { "✔ Show Free Space" } else { "Show Free Space" };
            item(ui, true, label, Action::ToggleFree);
        });

        // Tooltips.
        if let (Some(hi), Some(tree), false) = (self.hovered, &self.tree, resp.context_menu_opened()) {
            let held = self.hover_since.elapsed();
            let it = &self.items[hi];
            if held >= INFOTIP_DELAY {
                if let Some(e) = it.index.and_then(|i| tree.entry_at(&it.folder, i)) {
                    if let Some(pp) = resp.hover_pos() {
                        let screen = ui.ctx().content_rect();
                        // Flip to the other side of the pointer near the right/bottom edges.
                        let pivot = match (pp.x > screen.max.x - 260.0, pp.y > screen.max.y - 90.0) {
                            (false, false) => Align2::LEFT_TOP,
                            (true, false) => Align2::RIGHT_TOP,
                            (false, true) => Align2::LEFT_BOTTOM,
                            (true, true) => Align2::RIGHT_BOTTOM,
                        };
                        let off = Vec2::new(if pivot.x() == egui::Align::Min { 14.0 } else { -6.0 },
                                            if pivot.y() == egui::Align::Min { 18.0 } else { -6.0 });
                        egui::Area::new(Id::new("infotip"))
                            .order(Order::Tooltip)
                            .interactable(false)
                            .pivot(pivot)
                            .fixed_pos(pp + off)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                    ui.label(egui::RichText::new(&e.name).strong());
                                    ui.label(format::file_size(e.actual));
                                    ui.label(format::date(e.mtime));
                                });
                            });
                    }
                }
            } else {
                ui.ctx().request_repaint_after(INFOTIP_DELAY - held);
            }
        }

        // Zoom animation: one eased box, then switch views.
        if let Some(a) = &self.anim {
            let t = (a.start.elapsed().as_secs_f32() / ANIM_DURATION).min(1.0);
            if t >= 1.0 {
                self.finish_anim();
            } else {
                let e = 1.0 - (1.0 - t).powi(3);
                let r = Rect::from_min_max(a.from.min.lerp(a.to.min, e), a.from.max.lerp(a.to.max, e))
                    .translate(origin.to_vec2());
                painter.rect_filled(r, 0.0, pal.text.gamma_multiply(0.10));
                painter.rect_stroke(r, 0.0, Stroke::new(1.5, pal.text), egui::StrokeKind::Inside);
            }
            ui.ctx().request_repaint();
        }

        act
    }

    // -- dialogs -------------------------------------------------------------

    fn drive_dialog(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.dialog else { return };
        let mut chosen: Option<(Drive, bool)> = None;
        let mut close = false;
        let mut err = None;

        let modal = egui::Modal::new(Id::new("drive_dialog")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("Select Drive to View");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                if dlg.drives.is_empty() {
                    ui.label("No drives found.");
                }
                for (i, d) in dlg.drives.iter().enumerate() {
                    let used = d.total.saturating_sub(d.free);
                    let text = format!(
                        "{}    {} / {}  ({})    {}",
                        d.name,
                        format::size_string(used, 0, false),
                        format::size_string(d.total, 0, false),
                        format::size_string(used, d.total, true),
                        d.fs
                    );
                    let r = ui.add(egui::Button::selectable(dlg.selected == Some(i), text));
                    if r.clicked() {
                        dlg.selected = Some(i);
                    }
                    if r.double_clicked() {
                        chosen = Some((d.clone(), true));
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Folder:");
                let te = ui.add(egui::TextEdit::singleline(&mut dlg.path).desired_width(240.0));
                let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Browse...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Select Folder").pick_folder() {
                        dlg.path = p.display().to_string();
                    }
                }
                if (ui.add_enabled(!dlg.path.is_empty(), egui::Button::new("Open Folder")).clicked() || enter)
                    && !dlg.path.is_empty()
                {
                    match scan::drive_for_path(&PathBuf::from(dlg.path.trim())) {
                        Some(d) => chosen = Some((d, false)),
                        None => err = Some(format!("Not a readable folder: {}", dlg.path)),
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(dlg.selected.is_some(), egui::Button::new("OK")).clicked() {
                    if let Some(d) = dlg.selected.and_then(|i| dlg.drives.get(i)) {
                        chosen = Some((d.clone(), true));
                    }
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if modal.should_close() {
            close = true;
        }
        if err.is_some() {
            self.error = err;
        }
        if let Some((d, is_drive)) = chosen {
            self.dialog = None;
            self.start_scan(d, is_drive, ctx);
        } else if close {
            self.dialog = None;
        }
    }

    fn scan_dialog(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.scan else { return };
        let ctl = &job.ctl;
        let used = job.drive.total.saturating_sub(job.drive.free);
        let bytes = ctl.bytes.load(Ordering::Relaxed);
        let mut cancel = false;
        egui::Modal::new(Id::new("scan_dialog")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("Scanning Disk...");
            let mut cur = ctl.current.lock().map(|s| s.clone()).unwrap_or_default();
            if cur.chars().count() > 70 {
                let tail: String = cur.chars().rev().take(67).collect::<Vec<_>>().into_iter().rev().collect();
                cur = format!("...{tail}");
            }
            ui.label(cur);
            ui.label(format!("Files Found:  {}", ctl.files.load(Ordering::Relaxed)));
            ui.label(format!("Folders Found:  {}", ctl.folders.load(Ordering::Relaxed)));
            let frac = if used > 0 { (bytes as f32 / used as f32).min(1.0) } else { 0.0 };
            ui.add(egui::ProgressBar::new(frac).show_percentage());
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
        if cancel {
            ctl.cancel();
            self.scan = None;
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn message_dialogs(&mut self, ctx: &egui::Context) {
        if let Some((sel, path)) = self.confirm_delete.clone() {
            let mut answer = None;
            egui::Modal::new(Id::new("confirm_delete")).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.heading("Delete");
                ui.label(format!("Move to trash?\n\n{}", path.display()));
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
            if let Some(yes) = answer {
                self.confirm_delete = None;
                if yes {
                    self.delete_confirmed(sel, path);
                }
            }
        }
        if let Some(msg) = &self.error {
            let mut close = false;
            egui::Modal::new(Id::new("error")).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.heading("Error");
                ui.label(msg);
                if ui.button("OK").clicked() {
                    close = true;
                }
            });
            if close {
                self.error = None;
            }
        }
    }
}

impl eframe::App for SpaceMonger {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.applied_dark != Some(self.dark) {
            self.applied_dark = Some(self.dark);
            ctx.set_visuals(if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        }
        self.poll_scan();

        let mut act = egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui)).inner;
        let pal = self.palette();
        let tm = egui::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(pal.background))
            .show(ui, |ui| self.treemap(ui))
            .inner;
        act = act.or(tm);

        if self.dialog.is_none() && self.scan.is_none() && self.error.is_none() && self.confirm_delete.is_none() {
            let keys = ctx.input(|i| {
                if i.key_pressed(egui::Key::Backspace) {
                    Some(Action::ZoomOut)
                } else if i.key_pressed(egui::Key::Enter) {
                    Some(Action::ZoomIn)
                } else if i.key_pressed(egui::Key::Delete) {
                    Some(Action::Delete)
                } else if i.key_pressed(egui::Key::H) {
                    Some(Action::Hide)
                } else if i.key_pressed(egui::Key::F5) {
                    Some(Action::Reload)
                } else {
                    None
                }
            });
            act = act.or(keys);
        }
        if let Some(a) = act {
            self.run(a, &ctx);
        }

        self.drive_dialog(&ctx);
        self.scan_dialog(&ctx);
        self.message_dialogs(&ctx);

        let title = self.compute_title();
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette().background.to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("dark", self.dark.to_string());
        storage.set_string("show_free", self.show_free.to_string());
    }
}

// ---------------------------------------------------------------------------
// Drawing (port of FolderView.minimalDrawDisplayFolder & friends)

fn irect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect::from_min_size(Pos2::new(x as f32, y as f32), Vec2::new(w as f32, h as f32))
}

struct Draw<'a> {
    painter: &'a Painter,
    origin: Pos2,
    ppp: f32,
    pal: Palette,
    font: FontId,
}

impl Draw<'_> {
    fn snap(&self, p: Pos2) -> Pos2 {
        Pos2::new((p.x * self.ppp).round() / self.ppp, (p.y * self.ppp).round() / self.ppp)
    }

    fn rect(&self, x: i32, y: i32, w: i32, h: i32) -> Rect {
        let r = irect(x, y, w, h).translate(self.origin.to_vec2());
        Rect::from_min_max(self.snap(r.min), self.snap(r.max))
    }

    fn fill(&self, c: Color32, x: i32, y: i32, w: i32, h: i32) {
        if w > 0 && h > 0 {
            self.painter.rect_filled(self.rect(x, y, w, h), 0.0, c);
        }
    }

    fn text_width(&self, s: &str) -> (i32, i32) {
        let g = self.painter.layout_no_wrap(s.to_string(), self.font.clone(), Color32::WHITE);
        (g.size().x.ceil() as i32, g.size().y.ceil() as i32)
    }

    fn text(&self, p: &Painter, s: &str, x: i32, y: i32, c: Color32) {
        let pos = self.snap(self.origin + Vec2::new(x as f32, y as f32));
        p.text(pos, Align2::LEFT_TOP, s, self.font.clone(), c);
    }

    /// Flat box: a single fill with a 1px gap to its neighbours, plus its label.
    fn item(&self, tree: &Tree, it: &Item, sel: bool, hover: bool) {
        let pal = &self.pal;
        let (x, y, w, h) = (it.x, it.y, it.w + 1, it.h + 1);

        if !it.is_free {

            let color = if sel {
                pal.text
            } else if hover {
                pal.depth(it.depth).lerp_to_gamma(Color32::WHITE, 0.2)
            } else {
                pal.depth(it.depth)
            };

            
            self.fill(color, x + 1, y + 1, w - 2, h - 2);
            
            // draw border
            if w > 4 && h > 4 {
                
                let border_color = if sel {
                    pal.text
                } else if hover {
                    Color32::BLACK
                } else {
                    pal.border
                };

                let border_width = if hover { 2.0 } else { 1.0};

                // 1 physical pixel dark-grey border, just inside the fill.
                let r = self.rect(x + 1, y + 1, w - 2, h - 2);
                self.painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(border_width / self.ppp, border_color),
                    egui::StrokeKind::Inside,
                );
            }
        }

        if !it.labeled {
            return;
        }
        let Some(entry) = it.index.and_then(|i| tree.entry_at(&it.folder, i)) else { return };
        let p = self.painter.with_clip_rect(self.rect(x, y, w, h).intersect(self.painter.clip_rect()));
        let fg = if sel { pal.background } else { pal.text };

        if it.is_free {
            let ts = tree.total_space.max(1);
            let fp = tree.free_space as u128 * 1000 / ts as u128;
            let lines = [
                format!("<Free Space: {}.{}%>", fp / 10, fp % 10),
                format!("{} Free", format::size_string(tree.free_space, tree.total_space, false)),
                format!("Files Total:  {}", tree.num_files),
                format!("Folders Total:  {}", tree.num_folders),
            ];
            let (lw, lh) = self.text_width(&lines[0]);
            let tx = if lw > w - 2 { x + 2 } else { x + (w - lw) / 2 };
            let ty = if lh > h - 2 { y + 1 } else { y + (h - lh) / 2 };
            for (line, dy) in lines.iter().zip([-18, -6, 6, 15]) {
                self.text(&p, line, tx, ty + dy, pal.text);
            }
            return;
        }

        let (tw, th) = self.text_width(&entry.name);
        let tx = if tw > w - 2 || it.is_folder { x + 3 } else { x + (w - tw) / 2 };
        let mut ty = if th > h - 2 || it.is_folder { y + 2 } else { y + (h - th) / 2 };

        if !it.is_folder && h >= 36 && w >= 48 {
            for (s, dy) in [(format::file_size(entry.actual), 1), (format::date(entry.mtime), 11)] {
                let (sw, _) = self.text_width(&s);
                let sx = if sw > w - 2 { x + 3 } else { x + (w - sw) / 2 };
                self.text(&p, &s, sx, ty + dy, fg);
            }
            ty -= 12;
        }
        self.text(&p, &entry.name, tx, ty, fg);
    }

}
