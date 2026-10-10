//! Name / size / date tip shown when hovering a box.

use crate::core::model::Entry;
use crate::helpers::egui::popup_pivot;
use crate::ui::settings::Tooltips;
use crate::utils::format;
use eframe::egui::{self, Id, Order, Pos2, RichText, Vec2};
use std::path::Path;

/// Show the tip for `entry` (at `path` on disk) next to the pointer at `pos`.
pub fn infotip(ctx: &egui::Context, pos: Pos2, entry: &Entry, path: &Path, opts: &Tooltips) {
    let created = (opts.show_created && entry.created != 0).then_some(entry.created);
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
                // (label, value) rows under the name.
                let mut rows: Vec<(&str, String)> = Vec::new();
                if opts.show_path {
                    rows.push(("Path", path.display().to_string()));
                }
                if opts.show_size {
                    rows.push(("Size", format::file_size(entry.actual)));
                }
                if opts.show_modified {
                    rows.push(("Modified", format::date(entry.mtime)));
                }
                if let Some(c) = created {
                    rows.push(("Created", format::date(c)));
                }
                // A small two-column table, so every value says what it is and they line up.
                egui::Grid::new("infotip_rows")
                    .num_columns(2)
                    .spacing([10.0, ui.spacing().item_spacing.y])
                    .show(ui, |ui| {
                        for (label, value) in rows {
                            ui.label(text(label.to_string()));
                            ui.label(text(value));
                            ui.end_row();
                        }
                    });
            });
        });
}
