//! The treemap / path-bar font: egui's default, or a system font found with fontdb.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, OnceLock};

/// Font family used for treemap and path-bar labels.
pub fn map_family() -> FontFamily {
    FontFamily::Name("map".into())
}

/// Height of a folder's title bar for labels in `font`: 12px for egui's default font at 10px,
/// in proportion to the font's line height otherwise (fonts differ in how tall a line is).
pub fn title_height(ctx: &egui::Context, font: &egui::FontId) -> f64 {
    let base = egui::FontId::proportional(10.0);
    let (row, base_row) = ctx.fonts_mut(|f| (f.row_height(font), f.row_height(&base)));
    (crate::core::layout::TITLE_H * row as f64 / base_row as f64).ceil()
}

/// System fonts, loaded on first use (takes a moment on systems with many fonts).
fn database() -> &'static (fontdb::Database, Vec<String>) {
    static DB: OnceLock<(fontdb::Database, Vec<String>)> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let mut names: Vec<String> = db
            .faces()
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        (db, names)
    })
}

/// Family names of the installed fonts, sorted.
pub fn system_families() -> &'static [String] {
    &database().1
}

/// Regular face of a system family: font bytes and face index.
fn load_family(name: &str) -> Option<(Vec<u8>, u32)> {
    let db = &database().0;
    let id = db.query(&fontdb::Query {
        families: &[fontdb::Family::Name(name)],
        ..Default::default()
    })?;
    db.with_face_data(id, |data, index| (data.to_vec(), index))
}

/// Set up [`map_family`]: `family` (a system font name) first if it loads, then egui's
/// default fonts as fallback. An empty name keeps egui's default font.
pub fn apply(ctx: &egui::Context, family: &str) {
    let mut defs = FontDefinitions::default();
    let mut list = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if !family.is_empty() {
        if let Some((bytes, index)) = load_family(family) {
            let mut data = FontData::from_owned(bytes);
            data.index = index;
            defs.font_data.insert("user".into(), Arc::new(data));
            list.insert(0, "user".into());
        }
    }
    defs.families.insert(map_family(), list);
    ctx.set_fonts(defs);
    // That drops the preview fonts; they're added again when next shown.
    requested().clear();
}

/// Preview fonts asked for and not yet known to be loaded.
fn requested() -> std::sync::MutexGuard<'static, BTreeSet<String>> {
    static REQUESTED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    REQUESTED.lock().unwrap_or_else(|e| e.into_inner())
}

/// A font family that draws text in the system font `name`, for showing font names in their
/// own font. Loads the font on first use; until it's in egui's fonts (the next frame) this is
/// `None` and the caller uses the normal font.
pub fn preview_family(ctx: &egui::Context, name: &str) -> Option<FontFamily> {
    let family = FontFamily::Name(format!("preview:{name}").into());
    if ctx.fonts(|f| f.definitions().families.contains_key(&family)) {
        return Some(family);
    }
    if requested().insert(name.to_string()) {
        if let Some((bytes, index)) = load_family(name) {
            let mut data = FontData::from_owned(bytes);
            data.index = index;
            let to = InsertFontFamily {
                family,
                priority: FontPriority::Highest,
            };
            ctx.add_font(FontInsert::new(&format!("preview:{name}"), data, vec![to]));
            ctx.request_repaint();
        }
    }
    None
}
