//! The settings window: its own OS window (so it can sit outside the main one), editing
//! [`Settings`] in place so every change shows up in the main window at once.
//!
//! Like the main window it draws its own frame: no system title bar, a header to drag it by
//! with × on the right, resizable from its edges, and the main menu's rounded corners where a
//! compositor can show them (square without one). Pages are picked in a sidebar on the left.

use super::{form, pages, Settings};
use crate::ui::keymap::{same_shortcut, shortcut_text, Command, Keymap};
use crate::ui::widgets::dialog::{self, HEADER_H, PADDING};
use crate::ui::widgets::{titlebar, window_frame};
use eframe::egui::{
    self, Align, CornerRadius, Event, Id, Key, KeyboardShortcut, Layout, Margin, Rect, RichText,
    Sense, Stroke, UiBuilder, Vec2, ViewportBuilder, ViewportId,
};
use egui_phosphor::regular as icon;

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

const TITLE: &str = "Settings";
const SIDEBAR_W: f32 = 190.0;
const SIDEBAR_ITEM_H: f32 = 30.0;

/// Label column width on the Hotkeys page.
const KEY_LABEL_WIDTH: f32 = 230.0;

/// Which binding is waiting for a key press: (command, index in `binds` or `None` for new).
type Capture = (Command, Option<usize>);

pub struct SettingsWindow {
    pub open: bool,
    /// Frames spent trying to make the window a dialog of the main one (X11), or `None`
    /// once it is.
    attaching: Option<u32>,
    page: Page,
    capture: Option<Capture>,
}

impl Default for SettingsWindow {
    fn default() -> Self {
        Self {
            open: false,
            attaching: Some(0),
            page: Page::default(),
            capture: None,
        }
    }
}

impl SettingsWindow {
    /// Waiting for a shortcut: the main window should ignore key presses.
    pub fn capturing(&self) -> bool {
        self.open && self.capture.is_some()
    }

    fn close(&mut self) {
        self.open = false;
        self.capture = None;
    }

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
            .with_app_id("spacemonger")
            .with_decorations(false)
            .with_transparent(true)
            .with_inner_size([760.0, 600.0])
            .with_min_inner_size([560.0, 360.0]);
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("settings"),
            builder,
            |ui, _class| {
                if ui.input(|i| i.viewport().close_requested()) {
                    self.close();
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
                .max_rect(sidebar.shrink2(Vec2::new(10.0, 0.0)))
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

        titlebar::move_on_drag(ui, &drag, |p| !close_rect.contains(p));
        if close {
            self.close();
        }
        let esc = self.capture.is_none()
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
                ui.add_space(PADDING - 10.0);
                ui.add(egui::Label::new(RichText::new(TITLE).strong()).selectable(false));
            },
        );
        ui.spacing_mut().item_spacing.y = 2.0;
        for (group, pages) in SIDEBAR {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(PADDING - 10.0);
                ui.label(RichText::new(group).small().weak());
            });
            ui.add_space(2.0);
            for &(page, glyph, name) in pages {
                let text = (RichText::new(glyph).size(15.0), name);
                let b = egui::Button::selectable(self.page == page, text)
                    .min_size(Vec2::new(w, SIDEBAR_ITEM_H));
                if ui.add(b).clicked() && self.page != page {
                    self.page = page;
                    self.capture = None;
                }
            }
        }
    }

    fn page_ui(&mut self, ui: &mut egui::Ui, s: &mut Settings) {
        match self.page {
            Page::Navigation => pages::navigation(ui, s),
            Page::Scan => pages::scan(ui, s),
            Page::Actions => pages::actions(ui, s),
            Page::Hotkeys => self.keys_ui(ui, &mut s.keys),
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
        #[cfg(target_os = "linux")]
        let done = match crate::ui::x11_dialog::attach(TITLE) {
            crate::ui::x11_dialog::Attach::Pending => false,
            crate::ui::x11_dialog::Attach::Done(window) => {
                if let Some(w) = window {
                    crate::ui::x11_wm::add_shadow(w);
                }
                true
            }
        };
        #[cfg(not(target_os = "linux"))]
        let done = true;
        self.attaching = if done || tries > 120 {
            None
        } else {
            Some(tries + 1)
        };
        if self.attaching.is_some() {
            ctx.request_repaint();
        }
    }

    /// One row per command: its shortcuts (click one to change it, right-click to remove),
    /// "+" to add one, and a reset button.
    fn keys_ui(&mut self, ui: &mut egui::Ui, keys: &mut Keymap) {
        if let Some(cap) = self.capture {
            ui.label(RichText::new("Press a key combination… (Esc cancels)").strong());
            if let Some(sc) = captured(ui) {
                match (sc, cap) {
                    (None, _) => {}
                    (Some(sc), (cmd, Some(i))) => keys.binds[i] = (cmd, sc),
                    (Some(sc), (cmd, None)) => keys.binds.push((cmd, sc)),
                }
                self.capture = None;
            }
        } else {
            ui.weak("Click a shortcut to change it, right-click to remove it.");
        }
        ui.add_space(6.0);

        let mut remove = None;
        let mut reset = None;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        for (title, cmds) in Command::GROUPS {
            form::section(ui, title, |ui| {
                for &cmd in cmds {
                    let is_default = {
                        let now: Vec<_> = keys.for_command(cmd).map(|(_, s)| *s).collect();
                        let def = Keymap::defaults_for(cmd);
                        now.len() == def.len()
                            && now.iter().zip(&def).all(|(a, b)| same_shortcut(a, b))
                    };
                    let mut reset_clicked = false;
                    ui.push_id(cmd.label(), |ui| {
                        // Same columns as the other tabs: label, shortcuts, reset on the right.
                        form::line(
                            ui,
                            |ui| {
                                reset_clicked = ui
                                    .add_enabled(!is_default, egui::Button::new("⟲").small())
                                    .on_hover_text("Reset to default")
                                    .clicked();
                            },
                            |ui| {
                                let text = if is_default {
                                    RichText::new(cmd.label())
                                } else {
                                    RichText::new(cmd.label()).strong()
                                };
                                ui.label(text);
                                form::changed_dot(ui, !is_default);
                                form::pad_to(ui, KEY_LABEL_WIDTH);
                                let bound: Vec<(usize, KeyboardShortcut)> =
                                    keys.for_command(cmd).map(|(i, s)| (i, *s)).collect();
                                for (i, sc) in bound {
                                    let waiting = self.capture == Some((cmd, Some(i)));
                                    let clashes = keys.conflicts(i);
                                    let mut text = RichText::new(if waiting {
                                        "…".to_string()
                                    } else {
                                        shortcut_text(&sc)
                                    });
                                    if !clashes.is_empty() {
                                        text = text.color(ui.visuals().error_fg_color);
                                    }
                                    let mut b = ui.add(egui::Button::new(text).selected(waiting));
                                    if !clashes.is_empty() {
                                        let names: Vec<_> =
                                            clashes.iter().map(|c| c.label()).collect();
                                        b = b.on_hover_text(format!(
                                            "Also bound to: {}",
                                            names.join(", ")
                                        ));
                                    }
                                    if b.clicked() {
                                        self.capture = Some((cmd, Some(i)));
                                    }
                                    if b.secondary_clicked() {
                                        remove = Some(i);
                                    }
                                }
                                let adding = self.capture == Some((cmd, None));
                                if ui
                                    .add(
                                        egui::Button::new(if adding { "…" } else { "+" })
                                            .small()
                                            .selected(adding),
                                    )
                                    .on_hover_text("Add a shortcut")
                                    .clicked()
                                {
                                    self.capture = Some((cmd, None));
                                }
                            },
                        );
                    });
                    if reset_clicked {
                        reset = Some(cmd);
                    }
                }
            });
        }
        if let Some(i) = remove {
            keys.binds.remove(i);
            self.capture = None;
        }
        if let Some(cmd) = reset {
            keys.reset(cmd);
            self.capture = None;
        }
    }
}

/// A key pressed this frame while capturing: `Some(None)` for Esc (cancel),
/// `Some(Some(shortcut))` for anything else, `None` while still waiting.
fn captured(ui: &egui::Ui) -> Option<Option<KeyboardShortcut>> {
    ui.input_mut(|i| {
        let found = i.events.iter().find_map(|e| match e {
            Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } => Some((*key, *modifiers)),
            _ => None,
        })?;
        i.events.retain(|e| !matches!(e, Event::Key { .. }));
        Some(match found {
            (Key::Escape, m) if m.is_none() => None,
            (key, m) => Some(KeyboardShortcut::new(m, key)),
        })
    })
}

/// The main menu's corner radius where a compositor can show see-through corners, else none.
fn corner_radius(ui: &egui::Ui) -> CornerRadius {
    #[cfg(target_os = "linux")]
    let composited = crate::ui::x11_wm::composited();
    #[cfg(not(target_os = "linux"))]
    let composited = true;
    if composited {
        ui.visuals().menu_corner_radius
    } else {
        CornerRadius::ZERO
    }
}
