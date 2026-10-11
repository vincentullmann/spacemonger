//! The contents of the settings pages (all but Hotkeys, which the window draws itself).

use super::form::{self, group, rows};
use super::{General, Labels, Layout, PathBar, Scan, Settings, Text, Theme, Tiles, Tooltips};
use crate::constants::UI_ZOOM;
use crate::ui::palette::{Palette, Scheme};
use crate::utils::format::SizeFormat;
use eframe::egui::Ui;

const THEMES: [(Theme, &str); 3] = [
    (Theme::Light, "Light"),
    (Theme::Dark, "Dark"),
    (Theme::System, "Follow system"),
];

const SIZE_FORMATS: [(SizeFormat, &str); 5] = [
    (SizeFormat::Bytes, "Bytes (1,234,567 bytes)"),
    (SizeFormat::Binary, "Binary (1.2 MiB)"),
    (SizeFormat::Decimal, "Decimal (1.2 MB)"),
    (
        SizeFormat::BinaryAndBytes,
        "Binary + bytes (1.2 MiB (1,234,567 bytes))",
    ),
    (
        SizeFormat::DecimalAndBytes,
        "Decimal + bytes (1.2 MB (1,234,567 bytes))",
    ),
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

pub fn navigation(ui: &mut Ui, s: &mut Settings) {
    let (g, d) = (&mut s.general, General::default());
    rows(ui, |r| {
        r.row(
            "Animation",
            "Length of zoom and frame moves. 0 jumps straight there.",
            &mut g.anim_ms,
            &d.anim_ms,
            |ui, v| form::number(ui, v, 0..=2000, 10.0, "ms"),
        );
        r.row(
            "Scroll zoom speed",
            "How far one mouse-wheel step zooms, relative to the default.",
            &mut g.zoom_speed,
            &d.zoom_speed,
            |ui, v| form::number(ui, v, 10..=500, 5.0, "%"),
        );
    });
}

pub fn actions(ui: &mut Ui, s: &mut Settings) {
    let (g, d) = (&mut s.general, General::default());
    rows(ui, |r| {
        r.row(
            "Confirm before delete",
            "Ask before moving the selection to the trash.",
            &mut g.confirm_delete,
            &d.confirm_delete,
            form::check,
        );
        r.row(
            "Frame selection fill",
            "Framing the selection (F) zooms until its bounding box fills this much of the view.",
            &mut g.frame_fill,
            &d.frame_fill,
            |ui, v| form::number(ui, v, 10..=100, 1.0, "%"),
        );
    });
}

pub fn scan(ui: &mut Ui, s: &mut Settings) {
    let (sc, d) = (&mut s.scan, Scan::default());
    rows(ui, |r| {
        r.row(
            "Ignore hidden files",
            "Leave out files and folders whose name starts with a dot. Applies at once, no rescan needed.",
            &mut sc.ignore_hidden,
            &d.ignore_hidden,
            form::check,
        );
        r.row(
            "Stay on one filesystem",
            "Don't descend into other drives mounted inside the scanned folder.",
            &mut sc.one_filesystem,
            &d.one_filesystem,
            form::check,
        );
        r.row(
            "Count hard links once",
            "A file with several hard links counts once, at the first place it's found.",
            &mut sc.hardlinks_once,
            &d.hardlinks_once,
            form::check,
        );
        r.row(
            "Exclude patterns",
            "Files and folders to skip while scanning (not available yet).",
            &mut sc.excludes,
            &d.excludes,
            |ui, v| form::excludes(ui, v),
        );
    });
    ui.add_space(10.0);
    ui.weak("Filesystem and hard-link options apply from the next scan.");
}

pub fn appearance(ui: &mut Ui, s: &mut Settings) {
    let (g, dg) = (&mut s.general, General::default());
    let (f, d) = (&mut s.text, Text::default());
    rows(ui, |r| {
        r.row(
            "Theme",
            "Light or dark colours, or follow the desktop's preference.",
            &mut g.theme,
            &dg.theme,
            |ui, v| form::choice(ui, "theme", v, &THEMES),
        );
        r.row(
            "UI zoom",
            "Scales the whole window: text, buttons, bars and the map. Also Ctrl + / Ctrl - / Ctrl 0.",
            &mut g.ui_zoom,
            &dg.ui_zoom,
            |ui, v| form::number(ui, v, UI_ZOOM.0..=UI_ZOOM.1, 5.0, "%"),
        );
        r.row(
            "UI font size",
            "Text size of menus, dialogs, settings and the title bar. Map labels have their own.",
            &mut g.ui_font_size,
            &dg.ui_font_size,
            |ui, v| form::number(ui, v, 9.0..=20.0, 0.5, "px"),
        );
        r.row(
            "Map font",
            "Font for box labels and the path bar. Default is the built-in font.",
            &mut f.family,
            &d.family,
            form::font_family,
        );
    });
    group(ui, "Formats", |r| {
        r.row(
            "Size format",
            "How file sizes are shown in labels and tips. Totals (title bar, free space) use binary or decimal units.",
            &mut f.size_format,
            &d.size_format,
            |ui, v| form::choice(ui, "size_format", v, &SIZE_FORMATS),
        );
        r.row(
            "Date format",
            "strftime pattern for dates, e.g. %Y-%m-%d %H:%M. An invalid pattern falls back to the default.",
            &mut f.date_format,
            &d.date_format,
            |ui, v| ui.text_edit_singleline(v),
        );
    });
}

pub fn path_bar(ui: &mut Ui, s: &mut Settings) {
    let (p, d) = (&mut s.path_bar, PathBar::default());
    rows(ui, |r| {
        r.row(
            "Font size",
            "Text size in the path bar; the bar's height follows it.",
            &mut p.font_size,
            &d.font_size,
            |ui, v| form::number(ui, v, 6.0..=32.0, 0.5, "px"),
        );
    });
}

pub fn tiles(ui: &mut Ui, s: &mut Settings) {
    let before = s.tiles.clone();
    let (t, d) = (&mut s.tiles, Tiles::default());
    // The colours' default is the scheme's own set (Classic's for Custom).
    let scheme_colors = t
        .scheme
        .native_count()
        .and_then(|n| t.scheme.colors(n))
        .unwrap_or_else(|| d.colors.clone());
    group(ui, "Colours", |r| {
        r.row(
            "Colour scheme",
            "Where the depth colours come from. Picking one resets the colour list; editing a colour makes the scheme Custom.",
            &mut t.scheme,
            &d.scheme,
            |ui, v| form::choice(ui, "scheme", v, &SCHEMES),
        );
        r.row(
            "Colours",
            "One colour per nesting level, repeating. Click a swatch to edit it.",
            &mut t.colors,
            &scheme_colors,
            form::colors,
        );
        r.row(
            "Selection colour",
            "Fill and outline of selected boxes; their labels switch to black or white to stay readable.",
            &mut t.selection_color,
            &d.selection_color,
            |ui, v| form::auto_color(ui, v, Palette::TEXT, "default"),
        );
        r.row(
            "Hover highlight",
            "How much the box under the mouse lightens.",
            &mut t.hover,
            &d.hover,
            |ui, v| form::number(ui, v, 0..=100, 1.0, "%"),
        );
    });
    group(ui, "Outlines", |r| {
        r.row(
            "Borders",
            "Thin outline around every box. With a gap of 0, neighbours share one line.",
            &mut t.borders,
            &d.borders,
            form::check,
        );
        r.row(
            "Border",
            "Colour and width (in screen pixels) of the box outlines. Leave the colour unset for the standard grey.",
            &mut t.border,
            &d.border,
            |ui, v| form::outline(ui, v, Palette::BORDER, "default"),
        );
        r.row(
            "Hover border",
            "Colour and width of the outline of the box under the mouse.",
            &mut t.hover_border,
            &d.hover_border,
            |ui, v| form::outline(ui, v, Palette::HOVER_BORDER, "default"),
        );
        r.row(
            "Gap",
            "Space between neighbouring boxes.",
            &mut t.gap,
            &d.gap,
            |ui, v| form::number(ui, v, 0..=8, 1.0, "px"),
        );
    });
    // A colour list reset to the scheme's own stays with that scheme.
    if !(s.tiles.scheme == before.scheme && s.tiles.colors == scheme_colors) {
        s.tiles.reconcile(&before);
    }

    let (l, d) = (&mut s.layout, Layout::default());
    group(ui, "Layout", |r| {
        r.row(
            "Split bias",
            "Negative prefers side-by-side boxes, positive prefers stacked ones.",
            &mut l.bias,
            &d.bias,
            |ui, v| form::number(ui, v, -20..=20, 1.0, ""),
        );
        r.row(
            "Show free space",
            "Show the drive's free space as a box of its own (also in the main menu).",
            &mut l.show_free,
            &d.show_free,
            form::check,
        );
    });
}

pub fn tile_labels(ui: &mut Ui, s: &mut Settings) {
    let (lb, d) = (&mut s.labels, Labels::default());
    rows(ui, |r| {
        r.row(
            "Font size",
            "Text size of the labels in boxes. Folder title bars grow with it.",
            &mut lb.font_size,
            &d.font_size,
            |ui, v| form::number(ui, v, 6.0..=32.0, 0.5, "px"),
        );
        r.row(
            "Drop shadow",
            "Draw a soft shadow behind label text.",
            &mut lb.shadow,
            &d.shadow,
            form::check,
        );
        r.row(
            "Density",
            "Smallest box that gets a label (and shows a folder's contents): higher labels smaller boxes.",
            &mut lb.density,
            &d.density,
            |ui, v| form::number(ui, v, -3..=3, 1.0, ""),
        );
        r.row(
            "Elements",
            "What elements to show in the labels, as far as the box is big enough.",
            &mut lb.shown,
            &d.shown,
            |ui, v| form::shown(ui, "label_shown", v),
        );
    });
}

pub fn tooltips(ui: &mut Ui, s: &mut Settings) {
    let (tt, d) = (&mut s.tooltips, Tooltips::default());
    rows(ui, |r| {
        r.row(
            "Elements",
            "What elements to show in the tooltips.",
            &mut tt.shown,
            &d.shown,
            |ui, v| form::shown(ui, "tip_shown", v),
        );
        r.row(
            "Font size",
            "Text size in the tip.",
            &mut tt.font_size,
            &d.font_size,
            |ui, v| form::number(ui, v, 6.0..=32.0, 0.5, "px"),
        );
        r.row(
            "Delay",
            "How long to hover over a box before the tip pops up.",
            &mut tt.delay_ms,
            &d.delay_ms,
            |ui, v| form::number(ui, v, 0..=5000, 10.0, "ms"),
        );
    });
}
