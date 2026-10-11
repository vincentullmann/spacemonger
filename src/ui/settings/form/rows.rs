//! Labelled rows with a reset button, the layout every settings page shares.

use crate::ui::fonts;
use crate::ui::widgets::buttons::reset_button;
use eframe::egui::{self, Response, RichText, Ui};

/// Width of the label column, shared by every group so the inputs line up.
pub const LABEL_WIDTH: f32 = 170.0;
/// Space kept between the inputs and the reset button.
const RESET_GAP: f32 = 12.0;
/// Space above a section title.
const SECTION_GAP: f32 = 18.0;
/// Space between rows.
const ROW_SPACING: f32 = 6.0;
/// Size of the changed-value dot's slot, and the dot's radius.
const DOT_SLOT: f32 = 8.0;
const DOT_RADIUS: f32 = 3.0;

/// One settings line across the full width: `right` (the reset button) pinned to the right
/// edge, `left` (label and input) filling the rest from the left. Follows the window width.
pub fn line(ui: &mut Ui, right: impl FnOnce(&mut Ui), left: impl FnOnce(&mut Ui)) {
    let h = ui.spacing().interact_size.y;
    let size = egui::vec2(ui.available_width(), h);
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            right(ui);
            ui.add_space(RESET_GAP);
            let size = egui::vec2(ui.available_width(), h);
            ui.allocate_ui_with_layout(
                size,
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    // Keep the start of the line as the origin for `pad_to`.
                    ui.set_min_width(ui.available_width());
                    left(ui)
                },
            );
        },
    );
}

/// Space up to `x` from the start of the current line (the label column's width).
pub fn pad_to(ui: &mut Ui, x: f32) {
    let start = ui.min_rect().left();
    ui.add_space((start + x - ui.cursor().min.x).max(0.0));
}

/// The changed-value marker after a label. Its space is always taken, so the widgets after
/// it keep their ids (and focus / drag) when a value stops or starts being the default.
pub fn changed_dot(ui: &mut Ui, changed: bool) {
    let (dot, _) = ui.allocate_exact_size(egui::Vec2::splat(DOT_SLOT), egui::Sense::hover());
    if changed {
        let color = crate::ui::theme::accent(ui.visuals());
        ui.painter().circle_filled(dot.center(), DOT_RADIUS, color);
    }
}

/// A row's label: bold with the changed-value dot after it while `changed`.
pub fn label(ui: &mut Ui, text: &str, changed: bool) -> Response {
    let rich = if changed {
        RichText::new(text).strong()
    } else {
        RichText::new(text)
    };
    let r = ui.label(rich);
    changed_dot(ui, changed);
    r
}

/// A group of labelled rows.
pub struct Rows<'a> {
    ui: &'a mut Ui,
}

/// Rows without a title of their own (a page's only group: the page title names it).
pub fn rows(ui: &mut Ui, add: impl FnOnce(&mut Rows)) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = ROW_SPACING;
        add(&mut Rows { ui });
    });
}

/// A titled group of rows.
pub fn group(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Rows)) {
    section(ui, title, |ui| rows(ui, add));
}

/// A section: a title in the strong text colour with a rule under it.
pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(SECTION_GAP);
    ui.label(heading(title));
    ui.separator();
    add(ui);
}

impl Rows<'_> {
    /// A row for `v`: `label` on the left, the input `add` draws, and a reset button.
    /// A value that isn't `default` gets a bold label with a dot and an active reset button;
    /// right-click on the label or input offers the reset too. `tip` shows when hovering the
    /// label or input.
    pub fn row<T: PartialEq + Clone>(
        &mut self,
        label: &str,
        tip: &str,
        v: &mut T,
        default: &T,
        add: impl FnOnce(&mut Ui, &mut T) -> Response,
    ) {
        let changed = v != default;
        let reset = std::cell::Cell::new(false);
        let menu = |r: &Response| {
            r.context_menu(|ui| {
                if ui
                    .add_enabled(changed, egui::Button::new("Reset to default"))
                    .clicked()
                {
                    reset.set(true);
                    ui.close();
                }
            });
        };
        self.ui.push_id(label, |ui| {
            line(
                ui,
                |ui| {
                    if reset_button(ui, changed) {
                        reset.set(true);
                    }
                },
                |ui| {
                    let l = self::label(ui, label, changed).on_hover_text(tip);
                    menu(&l);
                    pad_to(ui, LABEL_WIDTH);
                    add(ui, v).on_hover_text(tip);
                },
            );
        });
        if reset.get() {
            *v = default.clone();
        }
    }
}

/// A section title: bold, in the strong text colour.
pub fn heading(title: &str) -> RichText {
    RichText::new(title).family(fonts::bold_family()).strong()
}
