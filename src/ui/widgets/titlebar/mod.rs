//! The window's own title bar (system decorations are off): command icons on the left, the
//! title (or scan progress) in the middle, minimise / maximise / close on the right. Empty
//! space drags the window, a double-click maximises it, a right-click opens the window
//! manager's window menu. The bar dims while the window isn't focused.

mod caption;
mod toolbar;
mod window_buttons;

use super::window_drag::move_or_maximize;
use super::CommandState;
use crate::core::actions::Action;
use crate::ui::keymap::Keymap;
use caption::{progress_line, search_placeholder, title_text};
use eframe::egui::{self, Align, Layout, Rect, Sense, Ui, UiBuilder, Vec2};
use toolbar::toolbar;
use window_buttons::window_buttons;

/// Height of the bar.
pub const HEIGHT: f32 = 34.0;
/// Size of the icon buttons.
const BUTTON: Vec2 = Vec2::new(30.0, 28.0);
/// Width of each of the window buttons on the right.
const WINDOW_BUTTON_W: f32 = 44.0;
/// The bar's opacity while the window isn't focused.
const UNFOCUSED_OPACITY: f32 = 0.55;
/// Width of the search placeholder.
const SEARCH_W: f32 = 260.0;
/// Thickness of the scan progress line.
const PROGRESS_H: f32 = 2.0;
/// Space between the icons and the title.
const TITLE_GAP: f32 = 12.0;
/// Space before the first icon.
const LEFT_PAD: f32 = 6.0;

/// Draw the bar. `progress` (0..=1) draws a progress line along its bottom edge. Returns the
/// command clicked, if any.
pub fn titlebar(
    ui: &mut Ui,
    st: &CommandState,
    keys: &Keymap,
    title: &str,
    progress: Option<f32>,
) -> Option<Action> {
    let full = ui.max_rect();
    if !ui.input(|i| i.viewport().focused.unwrap_or(true)) {
        ui.multiply_opacity(UNFOCUSED_OPACITY);
    }
    // Behind everything, so it only gets the pointer where nothing else is.
    let bg = ui.interact(full, ui.id().with("titlebar_bg"), Sense::click_and_drag());
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));

    // Window buttons first, so the icons give way (clipped) in a narrow window, not them.
    let buttons_w = 3.0 * WINDOW_BUTTON_W;
    let right = Rect::from_min_max(egui::pos2(full.right() - buttons_w, full.top()), full.max);
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(right)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            window_buttons(ui, maximized);
        },
    );

    let left = Rect::from_min_max(full.min, egui::pos2(right.left(), full.bottom()));
    let mut cmds = ui.new_child(
        UiBuilder::new()
            .max_rect(left)
            .layout(Layout::left_to_right(Align::Center)),
    );
    cmds.set_clip_rect(left.intersect(ui.clip_rect()));
    cmds.spacing_mut().item_spacing.x = 2.0;
    cmds.add_space(LEFT_PAD);
    let (act, search_hovered) = toolbar(&mut cmds, st, keys);

    let used = cmds.min_rect().right();
    let mid_left = (used + TITLE_GAP).min(left.right());
    let mid = Rect::from_min_max(
        egui::pos2(mid_left, full.top()),
        egui::pos2((left.right() - TITLE_GAP).max(mid_left), full.bottom()),
    );
    if search_hovered {
        search_placeholder(ui, mid, full);
    } else {
        title_text(ui, mid, full, title);
    }
    if let Some(frac) = progress {
        progress_line(ui, full, frac);
    }

    let icons = cmds.min_rect();
    let empty = |p: egui::Pos2| !icons.contains(p) && !right.contains(p);
    move_or_maximize(ui, &bg, empty, maximized);
    if bg.secondary_clicked() {
        if let Some(p) = bg.interact_pointer_pos().filter(|&p| empty(p)) {
            window_menu(ui, p);
        }
    }
    act
}

/// The window manager's own window menu (keep above, move to desktop, …), where it offers one.
fn window_menu(ui: &Ui, pos: egui::Pos2) {
    let px = (pos.to_vec2() * ui.ctx().pixels_per_point()).to_pos2();
    crate::ui::platform::show_window_menu(px);
}
