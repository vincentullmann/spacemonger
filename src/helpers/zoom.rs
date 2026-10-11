//! Keyboard control of the UI zoom setting.

use eframe::egui;

use crate::constants::UI_ZOOM;

/// UI zoom (percent) that Ctrl 0 goes back to.
const ZOOM_RESET: u32 = 100;
/// Step of Ctrl + / Ctrl -, in percent.
const ZOOM_STEP: i32 = 10;

/// Ctrl + / Ctrl - / Ctrl 0 in this viewport: step the UI zoom setting `z` (percent), within
/// [`UI_ZOOM`].
pub fn zoom_keys(ctx: &egui::Context, z: &mut u32) {
    use egui::gui_zoom::kb_shortcuts as kb;
    let step = |z: u32, d: i32| (z as i32 + d).clamp(UI_ZOOM.0 as i32, UI_ZOOM.1 as i32) as u32;
    ctx.input_mut(|i| {
        if i.consume_shortcut(&kb::ZOOM_RESET) {
            *z = ZOOM_RESET;
        } else if i.consume_shortcut(&kb::ZOOM_IN) || i.consume_shortcut(&kb::ZOOM_IN_SECONDARY) {
            *z = step(*z, ZOOM_STEP);
        } else if i.consume_shortcut(&kb::ZOOM_OUT) {
            *z = step(*z, -ZOOM_STEP);
        }
    });
}
