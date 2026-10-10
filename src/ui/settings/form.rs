//! Small building blocks for the settings pages: labelled rows with a reset button, and the input
//! widgets they hold (numbers with a unit, drop-downs, colour lists, ...).

use super::Shown;
use crate::ui::fonts;
use eframe::egui::{self, emath::Numeric, Color32, Response, RichText, Ui};
use std::ops::RangeInclusive;

/// Width of the label column, shared by every group so the inputs line up.
pub const LABEL_WIDTH: f32 = 170.0;
/// Space kept between the inputs and the reset button.
const RESET_GAP: f32 = 12.0;

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
    let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
    if changed {
        let color = ui.visuals().hyperlink_color;
        ui.painter().circle_filled(dot.center(), 3.0, color);
    }
}

/// A group of labelled rows.
pub struct Rows<'a> {
    ui: &'a mut Ui,
}

/// A titled group of rows.
pub fn group(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Rows)) {
    section(ui, title, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        add(&mut Rows { ui });
    });
}

/// A collapsible section: a title in the strong text colour (egui's heading colour is too
/// dim in dark mode) with a rule under it, open to begin with. Remembers being collapsed.
pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(8.0);
    let color = ui.visuals().strong_text_color();
    egui::CollapsingHeader::new(RichText::new(title).heading().color(color))
        .id_salt(("section", title))
        .default_open(true)
        .show_unindented(ui, |ui| {
            ui.separator();
            add(ui);
        });
}

/// A collapsible group of rows inside another group (e.g. Tiles / Labels).
fn subgroup(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Rows)) {
    ui.add_space(6.0);
    let color = ui.visuals().strong_text_color();
    egui::CollapsingHeader::new(RichText::new(title).strong().color(color))
        .id_salt(("subgroup", title))
        .default_open(true)
        .show_unindented(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            add(&mut Rows { ui });
        });
}

impl Rows<'_> {
    /// A collapsible group of rows nested in this one.
    pub fn subgroup(&mut self, title: &str, add: impl FnOnce(&mut Rows)) {
        subgroup(self.ui, title, add);
    }

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
                    let clicked = ui
                        .add_enabled(changed, egui::Button::new("⟲").small())
                        .on_hover_text("Reset to default")
                        .clicked();
                    if clicked {
                        reset.set(true);
                    }
                },
                |ui| {
                    let text = if changed {
                        RichText::new(label).strong()
                    } else {
                        RichText::new(label)
                    };
                    let l = ui.label(text).on_hover_text(tip);
                    menu(&l);
                    changed_dot(ui, changed);
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

/// A slider with its number field, and the unit after them.
pub fn number<T: Numeric>(
    ui: &mut Ui,
    v: &mut T,
    range: RangeInclusive<T>,
    step: f64,
    unit: &str,
) -> Response {
    let (lo, hi) = (range.start().to_f64(), range.end().to_f64());
    ui.spacing_mut().slider_width = 140.0;
    let r = ui.add(
        egui::Slider::new(v, range)
            .step_by(step)
            .drag_value_speed(step),
    );
    // Focused, the slider steps with the arrow keys itself; hovered, Up / Down do too.
    if r.hovered() && !r.has_focus() {
        let d = arrow_steps(ui);
        if d != 0 {
            *v = T::from_f64((v.to_f64() + d as f64 * step).clamp(lo, hi));
        }
    }
    if !unit.is_empty() {
        ui.weak(unit);
    }
    r
}

/// A drop-down choosing one of `options`.
pub fn choice<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: &str,
    v: &mut T,
    options: &[(T, &str)],
) -> Response {
    let shown = options.iter().find(|(o, _)| o == v).map_or("", |(_, l)| *l);
    let r = egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (o, label) in options {
                ui.selectable_value(v, *o, *label);
            }
        })
        .response;
    let i = options.iter().position(|(o, _)| o == v).unwrap_or(0);
    if let Some(j) = arrow_index(ui, &r, i, options.len()) {
        *v = options[j].0;
    }
    r
}

/// A drop-down of check boxes for which details to show; the button lists the ones on.
pub fn shown(ui: &mut Ui, id: &str, v: &mut Shown) -> Response {
    let mut copy = *v;
    let on: Vec<&str> = copy
        .fields()
        .into_iter()
        .filter(|(_, f)| **f)
        .map(|(l, _)| l)
        .collect();
    let text = if on.is_empty() {
        "Nothing".to_string()
    } else {
        on.join(", ")
    };
    egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .width(260.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            for (label, field) in v.fields() {
                ui.checkbox(field, label);
            }
        })
        .response
}

/// Up / Down presses this frame: +1 for each Up, -1 for each Down (consumed).
fn arrow_steps(ui: &Ui) -> i32 {
    ui.input_mut(|i| {
        let up = i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
        let down = i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
        up as i32 - down as i32
    })
}

/// For a closed drop-down that's hovered or focused: the option Up / Down moves to from
/// `i` (Down goes further down the list), if any.
fn arrow_index(ui: &Ui, r: &Response, i: usize, n: usize) -> Option<usize> {
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(r));
    if open || !(r.hovered() || r.has_focus()) || n == 0 {
        return None;
    }
    let d = -arrow_steps(ui);
    (d != 0).then(|| (i as i32 + d).clamp(0, n as i32 - 1) as usize)
}

/// A colour that's automatic until picked: shows `fallback` while it's `None` (with `hint`,
/// e.g. "theme"), and becomes `Some` once edited; reset goes back to automatic.
pub fn auto_color(ui: &mut Ui, v: &mut Option<Color32>, fallback: Color32, hint: &str) -> Response {
    let mut c = v.unwrap_or(fallback);
    let r = ui.color_edit_button_srgba(&mut c);
    if r.changed() {
        *v = Some(c);
    }
    if v.is_none() {
        ui.weak(hint);
    }
    r
}

/// One swatch per colour, plus buttons to drop / add one.
pub fn colors(ui: &mut Ui, colors: &mut Vec<Color32>) -> Response {
    ui.horizontal_wrapped(|ui| {
        // Buttons first so they stay put as the list grows; small swatches.
        if ui
            .add_enabled(colors.len() > 1, egui::Button::new("−").small())
            .on_hover_text("One colour fewer")
            .clicked()
        {
            colors.pop();
        }
        if ui
            .add_enabled(colors.len() < 64, egui::Button::new("+").small())
            .on_hover_text("One colour more")
            .clicked()
        {
            colors.push(colors.last().copied().unwrap_or(Color32::GRAY));
        }
        ui.spacing_mut().interact_size.x = 22.0;
        for c in colors.iter_mut() {
            ui.color_edit_button_srgba(c);
        }
    })
    .response
}

/// System font picker ("Default" is egui's built-in font).
pub fn font_family(ui: &mut Ui, family: &mut String) -> Response {
    let shown = if family.is_empty() {
        "Default"
    } else {
        family.as_str()
    };
    let r = egui::ComboBox::from_id_salt("font_family")
        .selected_text(shown.to_string())
        .height(f32::INFINITY)
        .show_ui(ui, |ui| {
            // Each name in its own font. Only the rows in view are drawn (and their fonts
            // loaded), so a long font list stays quick.
            let names = fonts::system_families();
            let size = ui.style().text_styles[&egui::TextStyle::Body].size + 2.0;
            let row_h = size + ui.spacing().button_padding.y * 2.0 + ui.spacing().item_spacing.y;
            egui::ScrollArea::vertical().max_height(400.0).show_rows(
                ui,
                row_h - ui.spacing().item_spacing.y,
                names.len() + 1,
                |ui, rows| {
                    for k in rows {
                        if k == 0 {
                            ui.selectable_value(family, String::new(), "Default");
                            continue;
                        }
                        let name = &names[k - 1];
                        let mut text = RichText::new(name).size(size);
                        if let Some(f) = fonts::preview_family(ui.ctx(), name) {
                            text = text.family(f);
                        }
                        ui.selectable_value(family, name.clone(), text);
                    }
                },
            );
        })
        .response;
    // "Default" is index 0, then the system fonts.
    let names = fonts::system_families();
    let i = names.iter().position(|n| n == family).map_or(0, |k| k + 1);
    if let Some(j) = arrow_index(ui, &r, i, names.len() + 1) {
        *family = if j == 0 {
            String::new()
        } else {
            names[j - 1].clone()
        };
    }
    r
}

/// Mock-up of the exclude list: not wired to the scanner yet.
pub fn excludes(ui: &mut Ui, list: &mut [String]) -> Response {
    ui.vertical(|ui| {
        ui.add_enabled_ui(false, |ui| {
            for p in list.iter_mut() {
                ui.text_edit_singleline(p);
            }
            for hint in ["node_modules", "*.tmp", "/proc"] {
                ui.add(egui::TextEdit::singleline(&mut String::new()).hint_text(hint));
            }
            let _ = ui.button("+ Add pattern");
        });
        ui.weak("Coming soon: glob patterns skipped while scanning.");
    })
    .response
}

/// A checkbox without a label of its own (the row has one).
pub fn check(ui: &mut Ui, v: &mut bool) -> Response {
    ui.checkbox(v, "")
}
