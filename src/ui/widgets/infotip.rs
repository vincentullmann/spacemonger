//! Name / size / date tip shown when hovering a box.

use crate::core::model::Entry;
use crate::helpers::egui::popup_pivot;
use crate::ui::settings::Tooltips;
use crate::utils::format;
use eframe::egui::{self, Id, Order, Pos2, RichText, Vec2};
use std::path::Path;

/// Show the tip for `entry` (at `path` on disk) next to the pointer at `pos`.
pub fn infotip(ctx: &egui::Context, pos: Pos2, entry: &Entry, path: &Path, opts: &Tooltips) {
    let k = opts.font_size / Tooltips::default().font_size;
    let (pivot, off) = popup_pivot(pos, ctx.content_rect(), Vec2::new(260.0, 90.0) * k);
    let text = |s: String| RichText::new(s).size(opts.font_size);
    egui::Area::new(Id::new("infotip"))
        .order(Order::Tooltip)
        .interactable(false)
        .pivot(pivot)
        .fixed_pos(pos + off)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(text(entry.name.clone()).strong());
                if opts.show_path {
                    ui.label(text(path.display().to_string()));
                }
                if opts.show_size {
                    ui.label(text(format::file_size(entry.actual)));
                }
                if opts.show_date {
                    ui.label(text(format::date(entry.mtime)));
                }
            });
        });
}
