//! The middle of the title bar: the window title, or where search will go, and the scan
//! progress line along the bottom.

use super::{BUTTON, PROGRESS_H, SEARCH_W};
use eframe::egui::{self, Rect, Ui, Vec2};
use egui_phosphor::regular as icon;

/// Narrowest space the title is drawn in.
const MIN_TITLE_W: f32 = 24.0;
/// Narrowest space the search placeholder is drawn in.
const MIN_SEARCH_W: f32 = 40.0;

/// While the search icon is hovered: a greyed-out search box where the title goes, to show
/// where search will live.
pub(super) fn search_placeholder(ui: &mut Ui, mid: Rect, full: Rect) {
    let w = SEARCH_W.min(mid.width());
    if w < MIN_SEARCH_W {
        return;
    }
    let h = BUTTON.y - 4.0;
    let cx = full
        .center()
        .x
        .clamp(mid.left() + w / 2.0, mid.right() - w / 2.0);
    let rect = Rect::from_center_size(egui::pos2(cx, full.center().y), Vec2::new(w, h));
    let mut text = String::new();
    let edit = egui::TextEdit::singleline(&mut text)
        .hint_text(format!("{}  Search (coming later)", icon::MAGNIFYING_GLASS))
        .desired_width(w);
    ui.put(rect, |ui: &mut Ui| ui.add_enabled(false, edit));
}

/// A thin line along the bottom of the bar, `frac` of the way across.
pub(super) fn progress_line(ui: &Ui, full: Rect, frac: f32) {
    let w = full.width() * frac.clamp(0.0, 1.0);
    let line = Rect::from_min_size(
        egui::pos2(full.left(), full.bottom() - PROGRESS_H),
        Vec2::new(w, PROGRESS_H),
    );
    ui.painter()
        .rect_filled(line, 0.0, crate::ui::theme::accent(ui.visuals()));
}

/// The window title in `mid`, centred on the whole bar (`full`) when it fits, cut
/// short with "…" when it doesn't.
pub(super) fn title_text(ui: &Ui, mid: Rect, full: Rect, title: &str) {
    if mid.width() < MIN_TITLE_W {
        return;
    }
    let color = ui.visuals().text_color();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple_singleline(title.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(mid.width());
    let galley = ui.painter().layout_job(job);
    let size = galley.size();
    let half = size.x / 2.0;
    let cx = full.center().x.clamp(mid.left() + half, mid.right() - half);
    let pos = egui::pos2(cx - half, full.center().y - size.y / 2.0).round();
    ui.painter().galley(pos, galley, color);
}
