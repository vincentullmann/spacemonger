//! Close, maximise / restore and minimise, drawn like the system's own.

use super::WINDOW_BUTTON_W;
use crate::constants::ICON_SIZE;
use eframe::egui::{self, Color32, Sense, Ui, Vec2, ViewportCommand};
use egui_phosphor::regular as icon;

/// Fill of a hovered close button.
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xc4, 0x2b, 0x1c);
/// How much a pressed button's fill darkens.
const PRESSED: f32 = 0.8;

/// Close, maximise / restore, minimise (laid out right to left).
pub(super) fn window_buttons(ui: &mut Ui, maximized: bool) {
    let h = ui.max_rect().height();
    let button = |ui: &mut Ui, glyph: &str, hover_fill: Option<Color32>, tip: &str| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(WINDOW_BUTTON_W, h), Sense::click());
        let visuals = ui.visuals();
        let mut color = visuals.text_color();
        if resp.hovered() {
            let fill = match hover_fill {
                Some(c) => {
                    color = Color32::WHITE;
                    c
                }
                None => visuals.widgets.hovered.weak_bg_fill,
            };
            let fill = if resp.is_pointer_button_down_on() {
                fill.gamma_multiply(PRESSED)
            } else {
                fill
            };
            ui.painter().rect_filled(rect, 0.0, fill);
        }
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(ICON_SIZE - 2.0),
            color,
        );
        resp.on_hover_text(tip).clicked()
    };
    if button(ui, icon::X, Some(CLOSE_HOVER), "Close") {
        ui.send_viewport_cmd(ViewportCommand::Close);
    }
    let (glyph, tip) = if maximized {
        (icon::COPY_SIMPLE, "Restore")
    } else {
        (icon::SQUARE, "Maximise")
    };
    if button(ui, glyph, None, tip) {
        ui.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    if button(ui, icon::MINUS, None, "Minimise") {
        ui.send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}
