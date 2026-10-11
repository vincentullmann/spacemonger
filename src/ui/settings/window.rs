//! The settings window: its own OS window (so it can sit outside the main one), editing
//! [`Settings`] in place so every change shows up in the main window at once.
//!
//! Like the main window it draws its own frame: no system title bar, a header to drag it by
//! with × on the right, resizable from its edges, and the main menu's rounded corners where a
//! compositor can show them (square without one). Pages are picked in a sidebar on the left.

use eframe::egui::{
    self, Align, CornerRadius, Id, Key, Layout, Margin, Rect, RichText, Sense, Stroke, UiBuilder,
    Vec2, ViewportBuilder, ViewportId,
};
use egui_phosphor::regular as icon;

use super::hotkeys::HotkeysPage;
use super::{pages, Settings};
use crate::constants::APP_ID;
use crate::helpers::zoom::zoom_keys;
use crate::ui::platform;
use crate::ui::widgets::dialog::{self, HEADER_H, PADDING};
use crate::ui::widgets::{move_on_drag, window_frame};

/// Window title (also how the window is found to make it a dialog, see [`platform`]).
const TITLE: &str = "Settings";
/// Window size to start with, and the smallest it can be resized to.
const SIZE: [f32; 2] = [760.0, 600.0];
const MIN_SIZE: [f32; 2] = [560.0, 360.0];
/// Sidebar width, its side padding, and the height of a page entry.
const SIDEBAR_W: f32 = 190.0;
const SIDEBAR_PAD: f32 = 10.0;
const SIDEBAR_ITEM_H: f32 = 30.0;
/// Size of the page icons in the sidebar.
const SIDEBAR_ICON: f32 = 15.0;
/// Frames to keep trying to make the window a dialog before giving up (about two seconds).
const ATTACH_TRIES: u32 = 120;

////////////////////////////////////////////////////////////////////////////////
// region:Pages

/// The settings pages, in sidebar order.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Page {
    #[default]
    Navigation,
    Scan,
    Actions,
    Hotkeys,
    Appearance,
    PathBar,
    Tiles,
    TileLabels,
    Tooltips,
}

/// A sidebar entry: the page, its icon and its name.
type Entry = (Page, &'static str, &'static str);

/// The sidebar: titled groups of pages.
const SIDEBAR: [(&str, &[Entry]); 2] = [
    (
        "General",
        &[
            (Page::Navigation, icon::COMPASS, "Navigation"),
            (Page::Scan, icon::HARD_DRIVES, "Scan"),
            (Page::Actions, icon::CURSOR_CLICK, "Actions"),
            (Page::Hotkeys, icon::KEYBOARD, "Hotkeys"),
        ],
    ),
    (
        "Display",
        &[
            (Page::Appearance, icon::PALETTE, "Appearance"),
            (Page::PathBar, icon::PATH, "Path bar"),
            (Page::Tiles, icon::SQUARES_FOUR, "Tiles"),
            (Page::TileLabels, icon::TAG, "Tile labels"),
            (Page::Tooltips, icon::CHAT_TEXT, "Tooltips"),
        ],
    ),
];

////////////////////////////////////////////////////////////////////////////////
// region:Window

/// The settings window's state (the settings themselves live in the app).
pub struct SettingsWindow {
    pub open: bool,
    /// Frames spent trying to make the window a dialog of the main one (X11), or `None`
    /// once it is.
    attaching: Option<u32>,
    page: Page,
    hotkeys: HotkeysPage,
}

impl Default for SettingsWindow {
    fn default() -> Self {
        Self {
            open: false,
            attaching: Some(0),
            page: Page::default(),
            hotkeys: HotkeysPage::default(),
        }
    }
}

impl SettingsWindow {
    /// Waiting for a shortcut: the main window should ignore key presses.
    pub fn capturing(&self) -> bool {
        self.open && self.hotkeys.capturing()
    }

    fn close(&mut self) {
        self.open = false;
        self.hotkeys.cancel();
    }

    /// Show the window while it's open, editing `s` in place.
    pub fn show(&mut self, ctx: &egui::Context, s: &mut Settings) {
        if !self.open {
            self.attaching = Some(0);
            return;
        }
        self.attach(ctx);
        let builder = ViewportBuilder::default()
            .with_title(TITLE)
            .with_window_type(egui::X11WindowType::Dialog)
            .with_taskbar(false)
            .with_app_id(APP_ID)
            .with_decorations(false)
            .with_transparent(true)
            .with_inner_size(SIZE)
            .with_min_inner_size(MIN_SIZE);
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("settings"),
            builder,
            |ui, _class| {
                if ui.input(|i| i.viewport().close_requested()) {
                    self.close();
                }
                if !self.hotkeys.capturing() {
                    zoom_keys(ui.ctx(), &mut s.general.ui_zoom);
                }
                self.window_ui(ui, s);
            },
        );
    }

    /// Everything inside the window: background, sidebar, header and the page.
    fn window_ui(&mut self, ui: &mut egui::Ui, s: &mut Settings) {
        let ctx = ui.ctx().clone();
        let full = ctx.content_rect();
        let r = corner_radius(ui);
        let visuals = ui.visuals().clone();
        let sidebar = Rect::from_min_size(full.min, Vec2::new(SIDEBAR_W, full.height()));
        let painter = ui.painter();
        painter.rect_filled(full, r, visuals.panel_fill);
        painter.rect_filled(
            sidebar,
            CornerRadius {
                nw: r.nw,
                sw: r.sw,
                ne: 0,
                se: 0,
            },
            visuals.faint_bg_color,
        );
        painter.vline(
            sidebar.right() - 0.5,
            sidebar.y_range(),
            Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
        );

        // The header strip moves the window. Registered first, so the widgets on it win.
        let strip = Rect::from_min_size(full.min, Vec2::new(full.width(), HEADER_H));
        let drag = ui.interact(strip, Id::new("settings_drag"), Sense::click_and_drag());

        let mut side = ui.new_child(
            UiBuilder::new()
                .max_rect(sidebar.shrink2(Vec2::new(SIDEBAR_PAD, 0.0)))
                .layout(Layout::top_down(Align::Min)),
        );
        self.sidebar_ui(&mut side);

        let content = Rect::from_min_max(egui::pos2(sidebar.right(), full.top()), full.max);
        let mut main = ui.new_child(
            UiBuilder::new()
                .max_rect(content)
                .layout(Layout::top_down(Align::Min)),
        );
        main.spacing_mut().item_spacing.y = 0.0;
        let title = SIDEBAR
            .iter()
            .flat_map(|(_, pages)| pages.iter())
            .find(|(p, _, _)| *p == self.page)
            .map_or("", |(_, _, name)| *name);
        let close = dialog::header(&mut main, title, true);
        let close_rect = Rect::from_min_size(
            egui::pos2(content.right() - HEADER_H, content.top()),
            Vec2::splat(HEADER_H),
        );
        egui::Frame::NONE
            .inner_margin(Margin {
                left: PADDING as i8,
                right: 4,
                top: 0,
                bottom: 4,
            })
            .show(&mut main, |ui| {
                ui.spacing_mut().item_spacing = ctx.global_style().spacing.item_spacing;
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        // Room for the scroll bar on the right.
                        egui::Frame::NONE
                            .inner_margin(Margin {
                                right: (PADDING - 4.0) as i8,
                                bottom: (PADDING - 4.0) as i8,
                                ..Margin::ZERO
                            })
                            .show(ui, |ui| self.page_ui(ui, s));
                    });
            });

        move_on_drag(ui, &drag, |p| !close_rect.contains(p));
        if close {
            self.close();
        }
        let esc = !self.hotkeys.capturing()
            && !egui::Popup::is_any_open(&ctx)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        if esc {
            self.close();
        }
        window_frame(&ctx, r.nw as f32);
    }

    fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let w = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(w, HEADER_H),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_size(Vec2::new(w, HEADER_H));
                ui.add_space(PADDING - SIDEBAR_PAD);
                ui.add(egui::Label::new(RichText::new(TITLE).strong()).selectable(false));
            },
        );
        ui.spacing_mut().item_spacing.y = 2.0;
        crate::ui::widgets::flat_items(ui);
        for (group, pages) in SIDEBAR {
            ui.add_space(SIDEBAR_PAD);
            ui.horizontal(|ui| {
                ui.add_space(PADDING - SIDEBAR_PAD);
                ui.weak(group);
            });
            ui.add_space(2.0);
            for &(page, glyph, name) in pages {
                let text = (RichText::new(glyph).size(SIDEBAR_ICON), name);
                let b = egui::Button::selectable(self.page == page, text)
                    .min_size(Vec2::new(w, SIDEBAR_ITEM_H));
                if ui.add(b).clicked() && self.page != page {
                    self.page = page;
                    self.hotkeys.cancel();
                }
            }
        }
    }

    fn page_ui(&mut self, ui: &mut egui::Ui, s: &mut Settings) {
        match self.page {
            Page::Navigation => pages::navigation(ui, s),
            Page::Scan => pages::scan(ui, s),
            Page::Actions => pages::actions(ui, s),
            Page::Hotkeys => self.hotkeys.ui(ui, &mut s.keys),
            Page::Appearance => pages::appearance(ui, s),
            Page::PathBar => pages::path_bar(ui, s),
            Page::Tiles => pages::tiles(ui, s),
            Page::TileLabels => pages::tile_labels(ui, s),
            Page::Tooltips => pages::tooltips(ui, s),
        }
    }

    /// Make the window a dialog of the main window (X11: in front of it, no taskbar entry,
    /// the main window's shadow). It only exists once egui has shown it, so keep trying for a
    /// little while.
    fn attach(&mut self, ctx: &egui::Context) {
        let Some(tries) = self.attaching else { return };
        let done = platform::attach_dialog(TITLE);
        self.attaching = if done || tries > ATTACH_TRIES {
            None
        } else {
            Some(tries + 1)
        };
        if self.attaching.is_some() {
            ctx.request_repaint();
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// region:Helpers

/// The main menu's corner radius where a compositor can show see-through corners, else none.
fn corner_radius(ui: &egui::Ui) -> CornerRadius {
    if platform::composited() {
        ui.visuals().menu_corner_radius
    } else {
        CornerRadius::ZERO
    }
}
