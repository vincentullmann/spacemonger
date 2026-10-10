//! The toolbar above the map.

use super::CommandState;
use crate::core::actions::Action;
use eframe::egui;

pub fn toolbar(ui: &mut egui::Ui, st: &CommandState) -> Option<Action> {
    let CommandState { has_tree, zoomed, sel_folder, has_sel, show_free, hidden, dark } = *st;
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
        let unhide = if hidden > 0 { format!("Unhide All ({hidden})") } else { "Unhide All".to_string() };
        btn(ui, hidden > 0, egui::Button::new(unhide), Action::UnhideAll);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            btn(ui, true, b(if dark { "☀" } else { "🌙" }), Action::ToggleDark);
        });
        act
    })
    .inner
}
