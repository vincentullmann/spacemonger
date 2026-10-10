//! The treemap / path-bar font: egui's default, or a system font found with fontdb.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::{Arc, OnceLock};

/// Font family used for treemap and path-bar labels.
pub fn map_family() -> FontFamily {
    FontFamily::Name("map".into())
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
}
