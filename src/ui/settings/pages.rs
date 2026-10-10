//! The contents of the General, Scan and Display tabs.

use super::form::{self, group};
use super::{Settings, Theme};
use crate::ui::palette::Scheme;
use eframe::egui::Ui;

const THEMES: [(Theme, &str); 3] = [
    (Theme::Light, "Light"),
    (Theme::Dark, "Dark"),
    (Theme::System, "Follow system"),
];

const SCHEMES: [(Scheme, &str); 10] = [
    (Scheme::Classic, "Classic"),
    (Scheme::Turbo, "Turbo"),
    (Scheme::Sinebow, "Sinebow"),
    (Scheme::Rainbow, "Rainbow"),
    (Scheme::Spectral, "Spectral"),
    (Scheme::Viridis, "Viridis"),
    (Scheme::Plasma, "Plasma"),
    (Scheme::Warm, "Warm"),
    (Scheme::Cool, "Cool"),
    (Scheme::Custom, "Custom"),
];

pub fn general(ui: &mut Ui, s: &mut Settings) {
    let g = &mut s.general;
    group(ui, "General", |r| {
        r.row("Theme", |ui| {
            form::choice(ui, "theme", &mut g.theme, &THEMES);
        });
        r.row("Animation", |ui| {
            form::number(ui, &mut g.anim_ms, 0..=2000, 10.0, "ms");
        });
        r.row("Scroll zoom speed", |ui| {
            form::number(ui, &mut g.zoom_speed, 10..=500, 5.0, "%");
        });
        r.row("Info tip delay", |ui| {
            form::number(ui, &mut g.infotip_ms, 0..=5000, 10.0, "ms");
        });
        r.row("Frame selection fill", |ui| {
            form::number(ui, &mut g.frame_fill, 10..=100, 1.0, "%");
        });
        r.row("Confirm before delete", |ui| {
            ui.checkbox(&mut g.confirm_delete, "");
        });
    });
    let l = &mut s.layout;
    group(ui, "Layout & labels", |r| {
        r.row("Label density", |ui| {
            form::number(ui, &mut l.density, -3..=3, 1.0, "");
        });
        r.row("Split bias", |ui| {
            form::number(ui, &mut l.bias, -20..=20, 1.0, "");
        });
        r.row("Show free space", |ui| {
            ui.checkbox(&mut l.show_free, "");
        });
        r.row("File size & date", |ui| {
            ui.checkbox(&mut l.file_details, "");
        });
        r.row("Decimal units (kB, MB)", |ui| {
            ui.checkbox(&mut l.decimal_units, "");
        });
        r.row("Date format", |ui| {
            ui.text_edit_singleline(&mut l.date_format);
        });
    });
}

pub fn scan(ui: &mut Ui, s: &mut Settings) {
    let sc = &mut s.scan;
    group(ui, "Scan", |r| {
        r.row("Ignore hidden files", |ui| {
            ui.checkbox(&mut sc.ignore_hidden, "");
        });
        r.row("Stay on one filesystem", |ui| {
            ui.checkbox(&mut sc.one_filesystem, "");
        });
        r.row("Count hard links once", |ui| {
            ui.checkbox(&mut sc.hardlinks_once, "");
        });
        r.row("Exclude patterns", |ui| {
            form::excludes(ui, &mut sc.excludes)
        });
    });
    ui.add_space(6.0);
    ui.weak("Filesystem and hard-link options apply from the next scan.");
}

pub fn display(ui: &mut Ui, s: &mut Settings) {
    let before = s.tiles.clone();
    let t = &mut s.tiles;
    group(ui, "Tiles", |r| {
        r.row("Colour scheme", |ui| {
            form::choice(ui, "scheme", &mut t.scheme, &SCHEMES);
        });
        r.row("Colours", |ui| form::colors(ui, &mut t.colors));
        r.row("Borders", |ui| {
            ui.checkbox(&mut t.borders, "");
        });
        r.row("Gap", |ui| {
            form::number(ui, &mut t.gap, 0..=8, 1.0, "px");
        });
        r.row("Hover highlight", |ui| {
            form::number(ui, &mut t.hover, 0..=100, 1.0, "%");
        });
    });
    s.tiles.reconcile(&before);

    let f = &mut s.font;
    group(ui, "Font", |r| {
        r.row("Family", |ui| {
            form::font_family(ui, &mut f.family);
        });
        r.row("Size", |ui| {
            form::number(ui, &mut f.size, 6.0..=32.0, 0.5, "px");
        });
        r.row("Drop shadow", |ui| {
            ui.checkbox(&mut f.shadow, "");
        });
    });

    let p = &mut s.path_bar;
    group(ui, "Path bar", |r| {
        r.row("Height", |ui| {
            form::number(ui, &mut p.height, 12.0..=48.0, 1.0, "px");
        });
        r.row("Font size", |ui| {
            form::number(ui, &mut p.font_size, 6.0..=32.0, 0.5, "px");
        });
    });
}
