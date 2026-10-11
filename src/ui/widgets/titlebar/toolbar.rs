//! The icons left of the title: the main menu, Open / Reload, and the zoom history and
//! search placeholders.

use super::BUTTON;
use crate::core::actions::Action;
use crate::ui::keymap::{shortcut_text, Command, Keymap};
use crate::ui::widgets::buttons::icon_button;
use crate::ui::widgets::menu::CommandMenu;
use crate::ui::widgets::{flat_items, CommandState};
use eframe::egui::{self, Sense, Stroke, Ui, Vec2};
use egui_phosphor::regular as icon;

/// Width of the main menu.
const MENU_W: f32 = 200.0;
/// Width of the space a divider takes, and how far its line stays off the bar's edges.
const DIVIDER_W: f32 = 11.0;
const DIVIDER_INSET: f32 = 7.0;

/// The icons, left to right. Also returns whether the search icon is hovered.
pub(super) fn toolbar(ui: &mut Ui, st: &CommandState, keys: &Keymap) -> (Option<Action>, bool) {
    let mut act = main_menu(ui, st, keys);
    for cmd in [Command::Open, Command::Reload] {
        let b = icon_button(cmd.icon(), BUTTON);
        let text = tip(keys, cmd);
        let resp = ui
            .add_enabled(st.enabled(cmd), b)
            .on_hover_text(&text)
            .on_disabled_hover_text(&text);
        if resp.clicked() {
            act = Some(cmd.action(false));
        }
    }
    divider(ui);

    // Placeholders for zoom history (see TODO.md).
    placeholder(ui, icon::ARROW_LEFT, "Back (coming later)");
    placeholder(ui, icon::ARROW_RIGHT, "Forward (coming later)");
    divider(ui);

    let search = ui.add_enabled(false, icon_button(icon::MAGNIFYING_GLASS, BUTTON));
    (act, search.contains_pointer())
}

/// A greyed-out icon for something still to come, with its tip.
fn placeholder(ui: &mut Ui, glyph: &str, tip: &str) {
    ui.add_enabled(false, icon_button(glyph, BUTTON))
        .on_disabled_hover_text(tip);
}

/// "Open… (Ctrl+O)": the command's name and its first shortcut.
fn tip(keys: &Keymap, cmd: Command) -> String {
    let name = cmd.menu_label();
    match keys.for_command(cmd).next() {
        Some((_, s)) => format!("{name}  ({})", shortcut_text(s)),
        None => name.to_owned(),
    }
}

/// A short vertical line between icon groups.
fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(DIVIDER_W, BUTTON.y), Sense::hover());
    let x = rect.center().x.round() + 0.5;
    let c = ui.visuals().widgets.noninteractive.bg_stroke.color;
    ui.painter().vline(
        x,
        rect.shrink2(Vec2::new(0.0, DIVIDER_INSET)).y_range(),
        Stroke::new(1.0, c),
    );
}

/// The menu behind the first icon: every command the bar has, grouped.
fn main_menu(ui: &mut Ui, st: &CommandState, keys: &Keymap) -> Option<Action> {
    let button = icon_button(icon::LIST, BUTTON);
    let mut menu = CommandMenu::new(keys, st, false);
    let (resp, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        ui.set_min_width(MENU_W);
        flat_items(ui);
        menu.items(ui, &[Command::Open, Command::Reload]);
        ui.separator();
        menu.items(ui, &[Command::ZoomIn, Command::ZoomOut, Command::ZoomFull]);
        ui.separator();
        menu.items(ui, &[Command::ToggleFree, Command::UnhideAll]);
        ui.separator();
        menu.item(ui, Command::Settings);
    });
    resp.on_hover_text("Menu");
    menu.clicked
}
