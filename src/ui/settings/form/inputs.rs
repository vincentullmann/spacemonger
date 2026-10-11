//! The input widgets settings rows hold: numbers with a unit, drop-downs, colours, fonts.

use std::ops::RangeInclusive;

use eframe::egui::{self, emath::Numeric, Color32, Response, RichText, Ui};
use strum::IntoEnumIterator;

use crate::ui::fonts;
use crate::ui::settings::{Line, Shown};
use crate::ui::widgets::buttons::add_button;
use crate::ui::widgets::flat_items;

/// Width of a number slider (its number field comes after it).
const SLIDER_W: f32 = 140.0;
/// Width of the "which details" drop-down.
const SHOWN_W: f32 = 260.0;
/// Width of a colour swatch in a colour list.
const SWATCH_W: f32 = 22.0;
/// Most colours a colour list takes.
const MAX_COLORS: usize = 64;
/// Space an automatic colour's hint takes (shown or not), so what follows stays put.
const HINT_W: f32 = 56.0;
/// Height of the font list before it scrolls.
const FONT_LIST_H: f32 = 400.0;
/// Font names in the font list are this much bigger than the UI text.
const FONT_PREVIEW_EXTRA: f32 = 2.0;

/// A slider with its number field, and the unit after them.
pub fn number<T: Numeric>(
    ui: &mut Ui,
    v: &mut T,
    range: RangeInclusive<T>,
    step: f64,
    unit: &str,
) -> Response {
    let (lo, hi) = (range.start().to_f64(), range.end().to_f64());
    ui.spacing_mut().slider_width = SLIDER_W;
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

/// A drop-down choosing one value of an enum, listed in declaration order under their
/// `strum` names.
pub fn choice<T>(ui: &mut Ui, id: &str, v: &mut T) -> Response
where
    T: PartialEq + Copy + IntoEnumIterator + Into<&'static str>,
{
    let options: Vec<T> = T::iter().collect();
    let name = |o: T| -> &'static str { o.into() };
    let r = egui::ComboBox::from_id_salt(id)
        .selected_text(name(*v))
        .show_ui(ui, |ui| {
            flat_items(ui);
            for &o in &options {
                ui.selectable_value(v, o, name(o));
            }
        })
        .response;
    let i = options.iter().position(|o| o == v).unwrap_or(0);
    if let Some(j) = arrow_index(ui, &r, i, options.len()) {
        *v = options[j];
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
        .width(SHOWN_W)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            flat_items(ui);
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
    // The hint takes the same width either way, so anything after it stays put.
    let x = ui.cursor().min.x;
    if v.is_none() {
        ui.weak(hint);
    }
    ui.add_space((x + HINT_W - ui.cursor().min.x).max(0.0));
    r
}

/// An outline's colour (automatic until picked) and width slider on one line.
pub fn outline(ui: &mut Ui, v: &mut Line, fallback: Color32, hint: &str) -> Response {
    let r = auto_color(ui, &mut v.color, fallback, hint);
    r | number(ui, &mut v.width, 0.0..=6.0, 0.5, "px")
}

/// One swatch per colour (click to edit, right-click to remove), then "+" to add one.
pub fn colors(ui: &mut Ui, colors: &mut Vec<Color32>) -> Response {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().interact_size.x = SWATCH_W;
        let mut remove = None;
        let only_one = colors.len() <= 1;
        for (i, c) in colors.iter_mut().enumerate() {
            let tip = if only_one {
                "Click to edit"
            } else {
                "Click to edit, right-click to remove"
            };
            let r = ui.color_edit_button_srgba(c).on_hover_text(tip);
            if r.secondary_clicked() && !only_one {
                remove = Some(i);
            }
        }
        if let Some(i) = remove {
            colors.remove(i);
        }
        if colors.len() < MAX_COLORS
            && add_button(ui, false)
                .on_hover_text("One colour more")
                .clicked()
        {
            colors.push(colors.last().copied().unwrap_or(Color32::GRAY));
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
            flat_items(ui);
            // Each name in its own font. Only the rows in view are drawn (and their fonts
            // loaded), so a long font list stays quick.
            let names = fonts::system_families();
            let size = ui.style().text_styles[&egui::TextStyle::Body].size + FONT_PREVIEW_EXTRA;
            let row_h = size + ui.spacing().button_padding.y * 2.0 + ui.spacing().item_spacing.y;
            egui::ScrollArea::vertical()
                .max_height(FONT_LIST_H)
                .show_rows(
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
