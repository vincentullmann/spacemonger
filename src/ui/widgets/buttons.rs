//! The small buttons several panels share: frameless icon buttons, ×, "+" and reset.

use eframe::egui::{Button, Response, RichText, Ui, Vec2};
use egui_phosphor::regular as icon;

use crate::constants::ICON_SIZE;
use crate::ui::widgets::flat_items;

/// Size of the × in dialog headers.
const CLOSE_SIZE: Vec2 = Vec2::splat(28.0);

/// A frameless button showing an icon glyph at [`ICON_SIZE`], at least `min_size` big: no
/// background until hovered.
pub fn icon_button(glyph: &str, min_size: Vec2) -> Button<'_> {
    Button::new(RichText::new(glyph).size(ICON_SIZE))
        .frame_when_inactive(false)
        .min_size(min_size)
}

/// A frameless ×, the size of the title bar's icon buttons.
pub fn close_button(ui: &mut Ui) -> Response {
    ui.add(icon_button(icon::X, CLOSE_SIZE))
        .on_hover_text("Close")
}

/// Space to keep right of [`close_button`] so its glyph lines up with `padding` from the edge.
pub fn close_button_margin(padding: f32) -> f32 {
    padding - (CLOSE_SIZE.x - ICON_SIZE) / 2.0
}

/// A small frameless "+" ("…" while `waiting`): no background until hovered, then a fill
/// without an outline.
pub fn add_button(ui: &mut Ui, waiting: bool) -> Response {
    ui.scope(|ui| {
        flat_items(ui);
        ui.add(
            Button::new(if waiting { "…" } else { "+" })
                .frame_when_inactive(false)
                .selected(waiting),
        )
    })
    .inner
}

/// The small "⟲" that puts a setting back to its default; active while `changed`. Returns
/// whether it was clicked.
pub fn reset_button(ui: &mut Ui, changed: bool) -> bool {
    ui.add_enabled(changed, Button::new("⟲").small())
        .on_hover_text("Reset to default")
        .clicked()
}
