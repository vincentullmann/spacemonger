//! Right-click menu on the map.

use super::CommandState;
use crate::core::actions::Action;
use eframe::egui;

pub fn context_menu(resp: &egui::Response, st: &CommandState) -> Option<Action> {
    let mut act = None;
    resp.context_menu(|ui| {
        let mut item = |ui: &mut egui::Ui, enabled: bool, label: &str, a: Action| {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                act = Some(a);
                ui.close();
            }
        };
        item(ui, st.sel_folder, "Zoom In", Action::ZoomIn);
        item(ui, st.zoomed, "Zoom Out", Action::ZoomOut);
        item(ui, st.zoomed, "Zoom Full", Action::ZoomFull);
        ui.separator();
        item(ui, st.has_sel, "Run / Open", Action::RunOpen);
        item(ui, st.has_sel, "Delete", Action::Delete);
        item(ui, st.has_sel, "Hide (H)", Action::Hide);
        item(ui, st.hidden > 0, "Unhide All (Shift+H)", Action::UnhideAll);
        ui.separator();
        item(ui, true, "Open Drive...", Action::Open);
        item(ui, true, "Rescan Drive", Action::Reload);
        let label = if st.show_free { "✔ Show Free Space" } else { "Show Free Space" };
        item(ui, true, label, Action::ToggleFree);
    });
    act
}
