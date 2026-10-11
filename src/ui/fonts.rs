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
        // Only fonts egui can draw (outline glyphs, not bitmap / colour-only ones) and that
        // cover the letters of their own name, so the picker can show each in its own font.
        names.retain(|n| shows_own_name(&db, n));
        (db, names)
    })
}

/// Whether the regular face of `name` has outline glyphs for every letter of the name.
fn shows_own_name(db: &fontdb::Database, name: &str) -> bool {
    use skrifa::raw::tables::cmap::PlatformId;
    use skrifa::raw::types::Tag;
    use skrifa::raw::TableProvider;
    use skrifa::{FontRef, GlyphNameSource, MetadataProvider};
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(name)],
        ..Default::default()
    };
    let Some(id) = db.query(&query) else {
        return false;
    };
    db.with_face_data(id, |data, index| {
        let Ok(font) = FontRef::from_index(data, index) else {
            return false;
        };
        let outlines = [b"glyf", b"CFF ", b"CFF2"]
            .iter()
            .any(|t| font.table_data(Tag::new(t)).is_some());
        // Symbol fonts map plain letters to pictures (Windows symbol cmap, or a Mac-only one).
        let symbol = font.cmap().is_ok_and(|cmap| {
            let recs = cmap.encoding_records();
            recs.iter()
                .any(|r| r.platform_id() == PlatformId::Windows && r.encoding_id() == 0)
                || !recs
                    .iter()
                    .any(|r| matches!(r.platform_id(), PlatformId::Windows | PlatformId::Unicode))
        });
        let charmap = font.charmap();
        // Others (like the URW dingbats) map letters to symbols under a Unicode cmap; their
        // glyph names give them away: the glyph for "a" isn't called "a…".
        let names = font.glyph_names();
        let letters: Vec<char> = name.chars().filter(char::is_ascii_alphabetic).collect();
        let misnamed = names.source() != GlyphNameSource::Synthesized
            && !letters.is_empty()
            && letters.iter().all(|&c| {
                let glyph = charmap.map(c);
                !glyph
                    .and_then(|g| names.get(g))
                    .is_some_and(|n| n.as_str().starts_with(c))
            });
        outlines
            && !symbol
            && !misnamed
            && name
                .chars()
                .filter(|c| !c.is_whitespace())
                .all(|c| charmap.map(c).is_some())
    })
    .unwrap_or(false)
}

/// Family names of the installed fonts that can show their own name, sorted.
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
    // Icon font for the title bar, as a fallback after the text font.
    egui_phosphor::add_to_fonts(&mut defs, egui_phosphor::Variant::Regular);
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
