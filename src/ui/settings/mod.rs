//! Settings kept between runs (eframe storage) and edited live in the settings window.

mod window;

pub use window::SettingsWindow;

use crate::constants::{ANIM_DURATION, BAR_H, FRAME_FILL, INFOTIP_DELAY};
use crate::core::camera::CameraParams;
use crate::core::fs::ScanOptions;
use crate::core::layout::LayoutParams;
use crate::ui::fonts;
use crate::ui::keymap::Keymap;
use crate::ui::palette::Scheme;
use crate::utils::format::{FormatOptions, DEFAULT_DATE_FORMAT};
use eframe::egui::{self, Color32, Response, Ui};
use egui_probe::{EguiProbe, Style};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Current storage key.
const KEY: &str = "settings.v2";
/// `{ dark, show_free }`, saved by the first `Settings`.
const KEY_V1: &str = "settings";
/// Keys used before `Settings` existed.
const LEGACY_KEYS: [&str; 2] = ["dark", "show_free"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Settings {
    pub general: General,
    pub layout: Layout,
    pub scan: Scan,
    pub tiles: Tiles,
    pub font: Font,
    pub path_bar: PathBar,
    pub keys: Keymap,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, EguiProbe)]
pub enum Theme {
    #[default]
    Light,
    Dark,
    /// Follow the desktop's light / dark preference.
    System,
}

impl From<Theme> for egui::ThemePreference {
    fn from(t: Theme) -> Self {
        match t {
            Theme::Light => Self::Light,
            Theme::Dark => Self::Dark,
            Theme::System => Self::System,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EguiProbe)]
#[serde(default)]
pub struct General {
    #[egui_probe(name = "Theme")]
    pub theme: Theme,
    #[egui_probe(name = "Animation (ms)", range = 0..=2000)]
    pub anim_ms: u32,
    #[egui_probe(name = "Scroll zoom speed (%)", range = 10..=500)]
    pub zoom_speed: u32,
    #[egui_probe(name = "Info tip delay (ms)", range = 0..=5000)]
    pub infotip_ms: u32,
    #[egui_probe(name = "Frame selection fill (%)", range = 10..=100)]
    pub frame_fill: u32,
    #[egui_probe(name = "Confirm before delete")]
    pub confirm_delete: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            anim_ms: (ANIM_DURATION * 1000.0) as u32,
            zoom_speed: 100,
            infotip_ms: INFOTIP_DELAY.as_millis() as u32,
            frame_fill: (FRAME_FILL * 100.0) as u32,
            confirm_delete: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EguiProbe)]
#[serde(default)]
pub struct Layout {
    /// Minimum box size for a label: -3 sparse .. +3 dense.
    #[egui_probe(name = "Label density", range = -3..=3)]
    pub density: i32,
    /// -20 prefers vertical splits, +20 horizontal.
    #[egui_probe(name = "Split bias", range = -20..=20)]
    pub bias: i32,
    #[egui_probe(name = "Show free space")]
    pub show_free: bool,
    #[egui_probe(name = "File size & date")]
    pub file_details: bool,
    #[egui_probe(name = "Decimal units (kB, MB)")]
    pub decimal_units: bool,
    #[egui_probe(name = "Date format (strftime)")]
    pub date_format: String,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            density: 0,
            bias: 0,
            show_free: true,
            file_details: true,
            decimal_units: false,
            date_format: DEFAULT_DATE_FORMAT.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EguiProbe)]
#[serde(default)]
pub struct Scan {
    /// Leave out files and folders named `.something` (applies at once).
    #[egui_probe(name = "Ignore hidden files")]
    pub ignore_hidden: bool,
    #[egui_probe(name = "Stay on one filesystem")]
    pub one_filesystem: bool,
    #[egui_probe(name = "Count hard links once")]
    pub hardlinks_once: bool,
    /// Not used by the scanner yet.
    #[egui_probe(name = "Exclude patterns", with excludes_probe)]
    pub excludes: Vec<String>,
}

impl Default for Scan {
    fn default() -> Self {
        Self {
            ignore_hidden: false,
            one_filesystem: true,
            hardlinks_once: true,
            excludes: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EguiProbe)]
#[serde(default)]
pub struct Tiles {
    #[egui_probe(name = "Colour scheme")]
    pub scheme: Scheme,
    /// One colour per nesting level, repeating.
    #[egui_probe(name = "Colours", with colors_probe)]
    pub colors: Vec<Color32>,
    #[egui_probe(name = "Borders")]
    pub borders: bool,
    #[egui_probe(name = "Gap (px)", range = 0..=8)]
    pub gap: u8,
    #[egui_probe(name = "Hover highlight (%)", range = 0..=100)]
    pub hover: u8,
}

impl Default for Tiles {
    fn default() -> Self {
        Self {
            scheme: Scheme::Classic,
            colors: Scheme::Classic.colors(8).unwrap_or_default(),
            borders: true,
            gap: 1,
            hover: 20,
        }
    }
}

impl Tiles {
    /// Keep scheme and colours consistent after an edit that started from `before`: a new
    /// scheme starts over with its own colour count, a new count resamples the scheme, and
    /// editing a colour makes it "Custom".
    pub fn reconcile(&mut self, before: &Tiles) {
        if self.scheme != before.scheme {
            if let Some(c) = self
                .scheme
                .native_count()
                .and_then(|n| self.scheme.colors(n))
            {
                self.colors = c;
            }
        } else if self.colors.len() != before.colors.len() {
            if let Some(c) = self.scheme.colors(self.colors.len().max(1)) {
                self.colors = c;
            }
        } else if self.colors != before.colors {
            self.scheme = Scheme::Custom;
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EguiProbe)]
#[serde(default)]
pub struct Font {
    /// System font family; empty for the built-in font.
    #[egui_probe(name = "Family", with family_probe)]
    pub family: String,
    #[egui_probe(name = "Size", range = 6.0..=32.0)]
    pub size: f32,
    #[egui_probe(name = "Drop shadow")]
    pub shadow: bool,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: String::new(),
            size: 10.0,
            shadow: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EguiProbe)]
#[serde(default)]
pub struct PathBar {
    #[egui_probe(name = "Height", range = 12.0..=48.0)]
    pub height: f32,
    #[egui_probe(name = "Font size", range = 6.0..=32.0)]
    pub font_size: f32,
}

impl Default for PathBar {
    fn default() -> Self {
        Self {
            height: BAR_H as f32,
            font_size: 10.0,
        }
    }
}

impl Settings {
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let Some(storage) = storage else {
            return Self::default();
        };
        eframe::get_value(storage, KEY).unwrap_or_else(|| Self::load_old(storage))
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, KEY, self);
        for k in LEGACY_KEYS.into_iter().chain([KEY_V1]) {
            storage.remove_string(k);
        }
    }

    /// Settings saved by older versions: the first `Settings`, or one string per key.
    fn load_old(storage: &dyn eframe::Storage) -> Self {
        #[derive(Deserialize)]
        struct V1 {
            dark: bool,
            show_free: bool,
        }
        let get = |k: &str| storage.get_string(k);
        let (dark, show_free) = match eframe::get_value::<V1>(storage, KEY_V1) {
            Some(v) => (v.dark, v.show_free),
            None => (
                get("dark").is_some_and(|v| v == "true"),
                get("show_free").is_none_or(|v| v == "true"),
            ),
        };
        let mut s = Self::default();
        s.general.theme = if dark { Theme::Dark } else { Theme::Light };
        s.layout.show_free = show_free;
        s
    }

    pub fn layout_params(&self, hide_dotfiles: bool) -> LayoutParams {
        LayoutParams {
            density: self.layout.density,
            bias: self.layout.bias,
            show_free: self.layout.show_free,
            hide_dotfiles,
        }
    }

    pub fn camera_params(&self) -> CameraParams {
        CameraParams {
            anim_duration: self.general.anim_ms as f32 / 1000.0,
            frame_fill: self.general.frame_fill as f64 / 100.0,
        }
    }

    pub fn scan_options(&self) -> ScanOptions {
        ScanOptions {
            one_filesystem: self.scan.one_filesystem,
            hardlinks_once: self.scan.hardlinks_once,
        }
    }

    pub fn format_options(&self) -> FormatOptions {
        FormatOptions {
            decimal_units: self.layout.decimal_units,
            date_format: self.layout.date_format.clone(),
        }
    }

    pub fn infotip_delay(&self) -> Duration {
        Duration::from_millis(self.general.infotip_ms as u64)
    }

    /// Zoom factor exponent per point of wheel scroll.
    pub fn wheel_zoom(&self) -> f64 {
        crate::constants::WHEEL_ZOOM * self.general.zoom_speed as f64 / 100.0
    }

    /// Label font for the treemap.
    pub fn map_font(&self) -> egui::FontId {
        egui::FontId::new(self.font.size, fonts::map_family())
    }

    /// Label font for the path bar.
    pub fn bar_font(&self) -> egui::FontId {
        egui::FontId::new(self.path_bar.font_size, fonts::map_family())
    }
}

/// Probe result helper: a response marked changed if `changed`.
fn mark(mut r: Response, changed: bool) -> Response {
    if changed {
        r.mark_changed();
    }
    r
}

/// One swatch per colour, plus buttons to drop / add one.
fn colors_probe(colors: &mut Vec<Color32>, ui: &mut Ui, _: &Style) -> Response {
    let mut changed = false;
    let r = ui
        .horizontal_wrapped(|ui| {
            // Buttons first so they stay put as the list grows; small swatches.
            if ui
                .add_enabled(colors.len() > 1, egui::Button::new("−").small())
                .on_hover_text("One colour fewer")
                .clicked()
            {
                colors.pop();
                changed = true;
            }
            if ui
                .add_enabled(colors.len() < 64, egui::Button::new("+").small())
                .on_hover_text("One colour more")
                .clicked()
            {
                colors.push(colors.last().copied().unwrap_or(Color32::GRAY));
                changed = true;
            }
            ui.spacing_mut().interact_size.x = 22.0;
            for c in colors.iter_mut() {
                changed |= ui.color_edit_button_srgba(c).changed();
            }
        })
        .response;
    mark(r, changed)
}

/// System font picker ("Default" is egui's built-in font).
fn family_probe(family: &mut String, ui: &mut Ui, _: &Style) -> Response {
    let mut changed = false;
    let shown = if family.is_empty() {
        "Default"
    } else {
        family.as_str()
    };
    let r = egui::ComboBox::from_id_salt("font_family")
        .selected_text(shown)
        .height(400.0)
        .show_ui(ui, |ui| {
            changed |= ui
                .selectable_value(family, String::new(), "Default")
                .changed();
            for name in fonts::system_families() {
                changed |= ui.selectable_value(family, name.clone(), name).changed();
            }
        })
        .response;
    mark(r, changed)
}

/// Mock-up of the exclude list: not wired to the scanner yet.
#[allow(clippy::ptr_arg)] // egui-probe's `with` signature
fn excludes_probe(list: &mut Vec<String>, ui: &mut Ui, _: &Style) -> Response {
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

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::Storage as _;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mem(HashMap<String, String>);

    impl eframe::Storage for Mem {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_string(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    fn with(theme: Theme, show_free: bool) -> Settings {
        let mut s = Settings::default();
        s.general.theme = theme;
        s.layout.show_free = show_free;
        s
    }

    #[test]
    fn round_trip_and_old_versions() {
        assert_eq!(Settings::load(None), Settings::default());
        let mut m = Mem::default();
        assert_eq!(Settings::load(Some(&m)), Settings::default());
        // Old string keys still load.
        m.set_string("dark", "true".into());
        m.set_string("show_free", "false".into());
        assert_eq!(Settings::load(Some(&m)), with(Theme::Dark, false));
        // So does the first Settings struct.
        m.set_string(KEY_V1, "(dark: false, show_free: false)".into());
        assert_eq!(Settings::load(Some(&m)), with(Theme::Light, false));
        // New key wins once saved, and the old ones go.
        let mut s = with(Theme::System, true);
        s.tiles.gap = 3;
        s.save(&mut m);
        assert_eq!(Settings::load(Some(&m)), s);
        assert!(m.get_string("dark").is_none() && m.get_string(KEY_V1).is_none());
        // Fields added later fall back to defaults.
        m.set_string(KEY, "(general: (theme: Dark))".into());
        assert_eq!(Settings::load(Some(&m)), with(Theme::Dark, true));
    }

    #[test]
    fn tiles_reconcile() {
        let before = Tiles::default();
        let mut t = before.clone();
        t.scheme = Scheme::Spectral;
        t.reconcile(&before);
        assert_eq!(t.colors, Scheme::Spectral.colors(11).unwrap());

        // A new scheme resets the count, even after colours were added.
        let before = t.clone();
        t.scheme = Scheme::Turbo;
        t.reconcile(&before);
        assert_eq!(t.colors, Scheme::Turbo.colors(8).unwrap());

        let before = t.clone();
        t.colors.push(Color32::RED);
        t.reconcile(&before);
        assert_eq!((t.scheme, t.colors.len()), (Scheme::Turbo, 9));

        let before = t.clone();
        t.colors[0] = Color32::RED;
        t.reconcile(&before);
        assert_eq!(t.scheme, Scheme::Custom);
    }
}
