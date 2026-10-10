//! Small building blocks for the settings pages: labelled rows in a grid, and the input
//! widgets they hold (numbers with a unit, drop-downs, colour lists, ...).

use crate::ui::fonts;
use eframe::egui::{self, emath::Numeric, Color32, Response, Ui};
use std::ops::RangeInclusive;

/// Width of the label column, shared by every group so the inputs line up.
const LABEL_WIDTH: f32 = 170.0;

/// A group of labelled rows.
pub struct Rows<'a> {
    ui: &'a mut Ui,
}

/// A titled group of rows.
pub fn group(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Rows)) {
    ui.add_space(8.0);
    ui.heading(title);
    ui.separator();
    egui::Grid::new(title)
        .num_columns(2)
        .min_col_width(LABEL_WIDTH)
        .spacing([12.0, 6.0])
        .show(ui, |ui| add(&mut Rows { ui }));
}

impl Rows<'_> {
    /// A row: `label` on the left, whatever `add` draws on the right; `tip` shows when
    /// hovering either.
    pub fn row(&mut self, label: &str, tip: &str, add: impl FnOnce(&mut Ui)) {
        self.ui.label(label).on_hover_text(tip);
        let rect = self.ui.horizontal(|ui| add(ui)).response.rect;
        // A hover-only area over the inputs, so the tip shows there too without taking
        // clicks away from them.
        self.ui
            .interact(rect, self.ui.id().with(label), egui::Sense::hover())
            .on_hover_text(tip);
        self.ui.end_row();
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
    let r = ui.add(egui::DragValue::new(v).range(range).speed(step));
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
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (o, label) in options {
                ui.selectable_value(v, *o, *label);
            }
        })
        .response
}

/// One swatch per colour, plus buttons to drop / add one.
pub fn colors(ui: &mut Ui, colors: &mut Vec<Color32>) {
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
    });
}

/// System font picker ("Default" is egui's built-in font).
pub fn font_family(ui: &mut Ui, family: &mut String) -> Response {
    let shown = if family.is_empty() {
        "Default"
    } else {
        family.as_str()
    };
    egui::ComboBox::from_id_salt("font_family")
        .selected_text(shown.to_string())
        .height(400.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(family, String::new(), "Default");
            for name in fonts::system_families() {
                ui.selectable_value(family, name.clone(), name);
            }
        })
        .response
}

/// Mock-up of the exclude list: not wired to the scanner yet.
pub fn excludes(ui: &mut Ui, list: &mut [String]) {
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
    });
}
