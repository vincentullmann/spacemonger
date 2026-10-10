//! Name / size / date tip shown when hovering a box.

use crate::core::model::Entry;
use crate::helpers::egui::popup_pivot;
use crate::utils::format;
use eframe::egui::{self, Id, Order, Pos2, Vec2};

/// Show the tip for `entry` next to the pointer at `pos`.
pub fn infotip(ctx: &egui::Context, pos: Pos2, entry: &Entry) {
    let (pivot, off) = popup_pivot(pos, ctx.content_rect(), Vec2::new(260.0, 90.0));
    egui::Area::new(Id::new("infotip"))
        .order(Order::Tooltip)
        .interactable(false)
        .pivot(pivot)
        .fixed_pos(pos + off)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(egui::RichText::new(&entry.name).strong());
                ui.label(format::file_size(entry.actual));
                ui.label(format::date(entry.mtime));
            });
        });
}
