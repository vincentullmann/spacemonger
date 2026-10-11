//! The Hotkeys page: one row per command with its shortcuts (click one to change it,
//! right-click to remove it), "+" to add one, and a reset button.

use eframe::egui::{self, KeyboardShortcut, RichText, Ui};

use super::form;
use crate::ui::keymap::{capture, same_shortcut, shortcut_text, Command, Keymap};
use crate::ui::widgets::buttons::{add_button, reset_button};

/// Label column width on this page (command names are longer than setting names).
const KEY_LABEL_WIDTH: f32 = 230.0;

/// Which binding is waiting for a key press: (command, index in `binds` or `None` for new).
type Capture = (Command, Option<usize>);

/// What a click on a row asked for, applied once the rows are drawn.
enum Edit {
    Capture(Capture),
    Remove(usize),
    Reset(Command),
}

/// The page's own state: the binding waiting for a key, if any.
#[derive(Default)]
pub struct HotkeysPage {
    capture: Option<Capture>,
}

impl HotkeysPage {
    /// Waiting for a shortcut: other key handling should stay out of the way.
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    /// Stop waiting for a shortcut (page or window closed).
    pub fn cancel(&mut self) {
        self.capture = None;
    }

    /// Draw the page and apply what was clicked or pressed to `keys`.
    pub fn ui(&mut self, ui: &mut Ui, keys: &mut Keymap) {
        self.header(ui, keys);
        ui.add_space(6.0);

        let mut edit = None;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        for (title, cmds) in Command::GROUPS {
            form::section(ui, title, |ui| {
                for &cmd in cmds {
                    if let Some(e) = self.command_row(ui, keys, cmd) {
                        edit = Some(e);
                    }
                }
            });
        }
        match edit {
            Some(Edit::Capture(c)) => self.capture = Some(c),
            Some(Edit::Remove(i)) => {
                keys.binds.remove(i);
                self.capture = None;
            }
            Some(Edit::Reset(cmd)) => {
                keys.reset(cmd);
                self.capture = None;
            }
            None => {}
        }
    }

    /// The hint line, or while capturing: the prompt, and the key once pressed.
    fn header(&mut self, ui: &mut Ui, keys: &mut Keymap) {
        let Some(cap) = self.capture else {
            ui.weak("Click a shortcut to change it, right-click to remove it.");
            return;
        };
        ui.label(RichText::new("Press a key combination… (Esc cancels)").strong());
        if let Some(pressed) = ui.input_mut(capture) {
            match (pressed, cap) {
                (None, _) => {}
                (Some(sc), (cmd, Some(i))) => keys.binds[i] = (cmd, sc),
                (Some(sc), (cmd, None)) => keys.binds.push((cmd, sc)),
            }
            self.capture = None;
        }
    }

    /// One command: label, its shortcuts, "+", and reset on the right (the same columns as
    /// the other pages).
    fn command_row(&self, ui: &mut Ui, keys: &Keymap, cmd: Command) -> Option<Edit> {
        let changed = !is_default(keys, cmd);
        let (mut edit, mut reset) = (None, false);
        ui.push_id(cmd.label(), |ui| {
            form::line(
                ui,
                |ui| reset = reset_button(ui, changed),
                |ui| {
                    form::label(ui, cmd.label(), changed);
                    form::pad_to(ui, KEY_LABEL_WIDTH);
                    let bound: Vec<(usize, KeyboardShortcut)> =
                        keys.for_command(cmd).map(|(i, s)| (i, *s)).collect();
                    for (i, sc) in bound {
                        if let Some(e) = self.shortcut_button(ui, keys, cmd, i, &sc) {
                            edit = Some(e);
                        }
                    }
                    let adding = self.capture == Some((cmd, None));
                    if add_button(ui, adding)
                        .on_hover_text("Add a shortcut")
                        .clicked()
                    {
                        edit = Some(Edit::Capture((cmd, None)));
                    }
                },
            );
        });
        if reset {
            edit = Some(Edit::Reset(cmd));
        }
        edit
    }

    /// One bound shortcut: click to change, right-click to remove; red with a tip when another
    /// command has the same one.
    fn shortcut_button(
        &self,
        ui: &mut Ui,
        keys: &Keymap,
        cmd: Command,
        i: usize,
        sc: &KeyboardShortcut,
    ) -> Option<Edit> {
        let waiting = self.capture == Some((cmd, Some(i)));
        let clashes = keys.conflicts(i);
        let mut text = RichText::new(if waiting {
            "…".to_string()
        } else {
            shortcut_text(sc)
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
            Some(Edit::Capture((cmd, Some(i))))
        } else if b.secondary_clicked() {
            Some(Edit::Remove(i))
        } else {
            None
        }
    }
}

/// `cmd` has exactly its default shortcuts.
fn is_default(keys: &Keymap, cmd: Command) -> bool {
    let now: Vec<_> = keys.for_command(cmd).map(|(_, s)| *s).collect();
    let def = Keymap::defaults_for(cmd);
    now.len() == def.len() && now.iter().zip(&def).all(|(a, b)| same_shortcut(a, b))
}
