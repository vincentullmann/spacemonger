//! The settings window: its own OS window (an in-app window where egui can't open one),
//! editing [`Settings`] in place so every change shows up in the main window at once.

use super::{pages, Settings};
use crate::ui::keymap::{same_shortcut, shortcut_text, Command, Keymap};
use eframe::egui::{self, Event, Key, KeyboardShortcut, RichText, ViewportBuilder, ViewportId};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    General,
    Scan,
    Display,
    Keys,
}

/// Label and shortcut column widths on the Keys tab.
const KEY_LABEL_WIDTH: f32 = 230.0;
const KEY_INPUT_WIDTH: f32 = 240.0;

/// Which binding is waiting for a key press: (command, index in `binds` or `None` for new).
type Capture = (Command, Option<usize>);

#[derive(Default)]
pub struct SettingsWindow {
    pub open: bool,
    tab: Tab,
    capture: Option<Capture>,
}

impl SettingsWindow {
    /// Waiting for a shortcut: the main window should ignore key presses.
    pub fn capturing(&self) -> bool {
        self.open && self.capture.is_some()
    }

    pub fn show(&mut self, ctx: &egui::Context, s: &mut Settings) {
        if !self.open {
            return;
        }
        let builder = ViewportBuilder::default()
            .with_title("Settings")
            .with_app_id("spacemonger")
            .with_inner_size([560.0, 640.0])
            .with_min_inner_size([420.0, 360.0]);
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("settings"),
            builder,
            |ui, _class| {
                if ui.input(|i| i.viewport().close_requested()) {
                    self.open = false;
                    self.capture = None;
                }
                egui::Panel::top("settings_tabs").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (t, label) in [
                            (Tab::General, "General"),
                            (Tab::Scan, "Scan"),
                            (Tab::Display, "Display"),
                            (Tab::Keys, "Keys"),
                        ] {
                            if ui.selectable_label(self.tab == t, label).clicked() {
                                self.tab = t;
                                self.capture = None;
                            }
                        }
                    });
                });
                egui::Panel::bottom("settings_footer").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                self.open = false;
                                self.capture = None;
                            }
                        });
                    });
                });
                egui::CentralPanel::default().show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| match self.tab {
                            Tab::General => pages::general(ui, s),
                            Tab::Scan => pages::scan(ui, s),
                            Tab::Display => pages::display(ui, s),
                            Tab::Keys => self.keys_ui(ui, &mut s.keys),
                        });
                });
            },
        );
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
            ui.add_space(8.0);
            ui.heading(title);
            ui.separator();
            for &cmd in cmds {
                let is_default = {
                    let now: Vec<_> = keys.for_command(cmd).map(|(_, s)| *s).collect();
                    let def = Keymap::defaults_for(cmd);
                    now.len() == def.len() && now.iter().zip(&def).all(|(a, b)| same_shortcut(a, b))
                };
                ui.horizontal(|ui| {
                    // Same columns as the other tabs: label, shortcuts, reset.
                    let x0 = ui.cursor().min.x;
                    if is_default {
                        ui.label(cmd.label());
                    } else {
                        ui.label(RichText::new(cmd.label()).strong());
                        let (dot, _) =
                            ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                        let color = ui.visuals().hyperlink_color;
                        ui.painter().circle_filled(dot.center(), 3.0, color);
                    }
                    ui.add_space((x0 + KEY_LABEL_WIDTH - ui.cursor().min.x).max(0.0));
                    let x1 = ui.cursor().min.x;
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
                            let names: Vec<_> = clashes.iter().map(|c| c.label()).collect();
                            b = b.on_hover_text(format!("Also bound to: {}", names.join(", ")));
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
                        .add(egui::Button::new(if adding { "…" } else { "+" }).selected(adding))
                        .on_hover_text("Add a shortcut")
                        .clicked()
                    {
                        self.capture = Some((cmd, None));
                    }
                    ui.add_space((x1 + KEY_INPUT_WIDTH - ui.cursor().min.x).max(0.0));
                    if ui
                        .add_enabled(!is_default, egui::Button::new("⟲").small())
                        .on_hover_text("Reset to default")
                        .clicked()
                    {
                        reset = Some(cmd);
                    }
                });
            }
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
