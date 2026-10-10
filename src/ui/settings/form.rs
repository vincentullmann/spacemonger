//! Small building blocks for the settings pages: labelled rows with a reset button, and the input
//! widgets they hold (numbers with a unit, drop-downs, colour lists, ...).

use crate::ui::fonts;
use eframe::egui::{self, emath::Numeric, Color32, Response, RichText, Ui};
use std::ops::RangeInclusive;

/// Width of the label column, shared by every group so the inputs line up.
const LABEL_WIDTH: f32 = 170.0;
/// Width of the input column; the reset buttons line up after it.
const INPUT_WIDTH: f32 = 300.0;

/// A group of labelled rows.
pub struct Rows<'a> {
    ui: &'a mut Ui,
}

/// A titled group of rows.
pub fn group(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Rows)) {
    ui.add_space(8.0);
    ui.heading(title);
    ui.separator();
    ui.spacing_mut().item_spacing.y = 6.0;
    add(&mut Rows { ui });
}

/// A group of rows belonging to the group above it (e.g. Tiles / Labels).
pub fn subgroup(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Rows)) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong());
    ui.spacing_mut().item_spacing.y = 6.0;
    add(&mut Rows { ui });
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
        self.ui.horizontal(|ui| {
            // Fixed-width label and input columns, padded with space so every row lines up.
            let x0 = ui.cursor().min.x;
            let l = if changed {
                let r = ui.label(RichText::new(label).strong());
                let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                let color = ui.visuals().hyperlink_color;
                ui.painter().circle_filled(dot.center(), 3.0, color);
                r
            } else {
                ui.label(label)
            }
            .on_hover_text(tip);
            menu(&l);
            ui.add_space((x0 + LABEL_WIDTH - ui.cursor().min.x).max(0.0));
            let x1 = ui.cursor().min.x;
            let inner = ui.scope(|ui| add(ui, v));
            ui.add_space((x1 + INPUT_WIDTH - ui.cursor().min.x).max(0.0));
            menu(&inner.inner);
            // A hover-only area over the inputs, so the tip shows there too without taking
            // clicks away from them.
            ui.interact(
                inner.response.rect,
                ui.id().with(label),
                egui::Sense::hover(),
            )
            .on_hover_text(tip);
            let clicked = ui
                .add_enabled(changed, egui::Button::new("⟲").small())
                .on_hover_text("Reset to default")
                .clicked();
            if clicked {
                reset.set(true);
            }
        });
        if reset.get() {
            *v = default.clone();
        }
    }
}

/// A number field with its unit after it.
pub fn number<T: Numeric>(
    ui: &mut Ui,
    v: &mut T,
    range: RangeInclusive<T>,
    step: f64,
    unit: &str,
) -> Response {
    let (lo, hi) = (range.start().to_f64(), range.end().to_f64());
    let r = ui.add(egui::DragValue::new(v).range(range).speed(step));
    // Focused, the field steps with Up / Down itself; hovered, we do the same.
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
        .height(400.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(family, String::new(), "Default");
            for name in fonts::system_families() {
                ui.selectable_value(family, name.clone(), name);
            }
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
