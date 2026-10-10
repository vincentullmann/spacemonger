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
        r.row(
            "Theme",
            "Light or dark colours, or follow the desktop's preference.",
            |ui| {
                form::choice(ui, "theme", &mut g.theme, &THEMES);
            },
        );
        r.row(
            "Animation",
            "Length of zoom and frame moves. 0 jumps straight there.",
            |ui| {
                form::number(ui, &mut g.anim_ms, 0..=2000, 10.0, "ms");
            },
        );
        r.row(
            "Scroll zoom speed",
            "How far one mouse-wheel step zooms, relative to the default.",
            |ui| {
                form::number(ui, &mut g.zoom_speed, 10..=500, 5.0, "%");
            },
        );
        r.row(
            "Info tip delay",
            "How long to hover over a box before its name, size and date pop up.",
            |ui| {
                form::number(ui, &mut g.infotip_ms, 0..=5000, 10.0, "ms");
            },
        );
        r.row(
            "Frame selection fill",
            "Framing the selection (F) zooms until its bounding box fills this much of the view.",
            |ui| {
                form::number(ui, &mut g.frame_fill, 10..=100, 1.0, "%");
            },
        );
        r.row(
            "Confirm before delete",
            "Ask before moving the selection to the trash.",
            |ui| {
                ui.checkbox(&mut g.confirm_delete, "");
            },
        );
    });
    let l = &mut s.layout;
    group(ui, "Layout & labels", |r| {
        r.row(
            "Label density",
            "Smallest box that gets a label (and shows a folder's contents): higher shows more, smaller labels.",
            |ui| {
            form::number(ui, &mut l.density, -3..=3, 1.0, "");
        });
        r.row(
            "Split bias",
            "Negative prefers side-by-side boxes, positive prefers stacked ones.",
            |ui| {
                form::number(ui, &mut l.bias, -20..=20, 1.0, "");
            },
        );
        r.row(
            "Show free space",
            "Show the drive's free space as a box of its own (also the Free Space button).",
            |ui| {
                ui.checkbox(&mut l.show_free, "");
            },
        );
        r.row(
            "File size & date",
            "Show the size and modification date under file names, where the box is big enough.",
            |ui| {
                ui.checkbox(&mut l.file_details, "");
            },
        );
        r.row(
            "Decimal units (kB, MB)",
            "Sizes in powers of 1000 (kB, MB) instead of 1024 (KiB, MiB).",
            |ui| {
                ui.checkbox(&mut l.decimal_units, "");
            },
        );
        r.row(
            "Date format",
            "strftime pattern for dates, e.g. %Y-%m-%d %H:%M. An invalid pattern falls back to the default.",
            |ui| {
            ui.text_edit_singleline(&mut l.date_format);
        });
    });
}

pub fn scan(ui: &mut Ui, s: &mut Settings) {
    let sc = &mut s.scan;
    group(ui, "Scan", |r| {
        r.row(
            "Ignore hidden files",
            "Leave out files and folders whose name starts with a dot. Applies at once, no rescan needed.",
            |ui| {
            ui.checkbox(&mut sc.ignore_hidden, "");
        });
        r.row(
            "Stay on one filesystem",
            "Don't descend into other drives mounted inside the scanned folder.",
            |ui| {
                ui.checkbox(&mut sc.one_filesystem, "");
            },
        );
        r.row(
            "Count hard links once",
            "A file with several hard links counts once, at the first place it's found.",
            |ui| {
                ui.checkbox(&mut sc.hardlinks_once, "");
            },
        );
        r.row(
            "Exclude patterns",
            "Files and folders to skip while scanning (not available yet).",
            |ui| form::excludes(ui, &mut sc.excludes),
        );
    });
    ui.add_space(6.0);
    ui.weak("Filesystem and hard-link options apply from the next scan.");
}

pub fn display(ui: &mut Ui, s: &mut Settings) {
    let before = s.tiles.clone();
    let t = &mut s.tiles;
    group(ui, "Tiles", |r| {
        r.row(
            "Colour scheme",
            "Where the depth colours come from. Picking one resets the colour list; editing a colour makes the scheme Custom.",
            |ui| {
            form::choice(ui, "scheme", &mut t.scheme, &SCHEMES);
        });
        r.row(
            "Colours",
            "One colour per nesting level, repeating. Click a swatch to edit it.",
            |ui| form::colors(ui, &mut t.colors),
        );
        r.row(
            "Borders",
            "Thin outline around every box. With a gap of 0, neighbours share one line.",
            |ui| {
                ui.checkbox(&mut t.borders, "");
            },
        );
        r.row("Gap", "Space between neighbouring boxes.", |ui| {
            form::number(ui, &mut t.gap, 0..=8, 1.0, "px");
        });
        r.row(
            "Hover highlight",
            "How much the box under the mouse lightens.",
            |ui| {
                form::number(ui, &mut t.hover, 0..=100, 1.0, "%");
            },
        );
    });
    s.tiles.reconcile(&before);

    let f = &mut s.font;
    group(ui, "Font", |r| {
        r.row(
            "Family",
            "Font for box labels and the path bar. Default is the built-in font.",
            |ui| {
                form::font_family(ui, &mut f.family);
            },
        );
        r.row("Size", "Label text size in boxes.", |ui| {
            form::number(ui, &mut f.size, 6.0..=32.0, 0.5, "px");
        });
        r.row(
            "Drop shadow",
            "Draw a soft shadow behind label text.",
            |ui| {
                ui.checkbox(&mut f.shadow, "");
            },
        );
    });

    let p = &mut s.path_bar;
    group(ui, "Path bar", |r| {
        r.row("Height", "Height of the path bar above the map.", |ui| {
            form::number(ui, &mut p.height, 12.0..=48.0, 1.0, "px");
        });
        r.row("Font size", "Text size in the path bar.", |ui| {
            form::number(ui, &mut p.font_size, 6.0..=32.0, 0.5, "px");
        });
    });
}
