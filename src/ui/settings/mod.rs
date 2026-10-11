//! Settings kept between runs (eframe storage) and edited live in the settings window.

mod defaults;
mod form;
mod pages;
mod window;

pub use window::SettingsWindow;

use crate::constants::{BAR_H, WHEEL_ZOOM};
use crate::core::camera::CameraParams;
use crate::core::fs::ScanOptions;
use crate::core::layout::LayoutParams;
use crate::ui::fonts;
use crate::ui::keymap::Keymap;
use crate::ui::palette::Scheme;
use crate::utils::format::{FormatOptions, SizeFormat};
use eframe::egui::{self, Color32};
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
    pub labels: Labels,
    #[serde(alias = "font")]
    pub text: Text,
    pub path_bar: PathBar,
    pub tooltips: Tooltips,
    pub keys: Keymap,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct General {
    pub theme: Theme,
    /// Zoom of the whole window, in percent (also Ctrl + / - / 0).
    pub ui_zoom: u32,
    /// Size of the UI's body text in px; the other text styles follow.
    pub ui_font_size: f32,
    pub anim_ms: u32,
    pub zoom_speed: u32,
    pub frame_fill: u32,
    pub confirm_delete: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Layout {
    /// -20 prefers vertical splits, +20 horizontal.
    pub bias: i32,
    pub show_free: bool,
}

/// Text inside the tiles.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Labels {
    pub font_size: f32,
    pub shadow: bool,
    /// Minimum box size for a label: -3 sparse .. +3 dense.
    pub density: i32,
    /// What file labels show.
    pub shown: Shown,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Scan {
    /// Leave out files and folders named `.something` (applies at once).
    pub ignore_hidden: bool,
    pub one_filesystem: bool,
    pub hardlinks_once: bool,
    /// Not used by the scanner yet.
    pub excludes: Vec<String>,
}

/// An outline: its colour (`None` = automatic) and width in physical pixels.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub color: Option<Color32>,
    pub width: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Tiles {
    pub scheme: Scheme,
    /// One colour per nesting level, repeating.
    pub colors: Vec<Color32>,
    /// Label and folder title colour, used as is.
    pub text_color: Color32,
    /// Outline of every box; width 0 for none.
    pub border: Line,
    /// Outline of the box under the mouse.
    pub hover_border: Line,
    /// Fill and outline of selected boxes; `None` follows the theme.
    pub selection_color: Option<Color32>,
    pub gap: u8,
    pub hover: u8,
    /// The old "Borders" switch, read from saved settings only (see [`Settings::load`]).
    #[serde(rename = "borders", skip_serializing, deserialize_with = "some_bool")]
    pub legacy_borders: Option<bool>,
}

/// `after` is `before` with one colour taken out.
fn removed_one(before: &[Color32], after: &[Color32]) -> bool {
    after.len() + 1 == before.len()
        && (0..before.len()).any(|i| before[..i] == after[..i] && before[i + 1..] == after[i..])
}

/// A plain `bool` read into an `Option` (RON wants `Some(..)` for an `Option` otherwise).
fn some_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    bool::deserialize(d).map(Some)
}

impl Tiles {
    /// Keep scheme and colours consistent after an edit that started from `before`: a new
    /// scheme starts over with its own colour count, a colour added resamples the scheme,
    /// and removing or editing a colour makes it "Custom" (unless that's the scheme's own).
    pub fn reconcile(&mut self, before: &Tiles) {
        if self.scheme != before.scheme {
            if let Some(c) = self
                .scheme
                .native_count()
                .and_then(|n| self.scheme.colors(n))
            {
                self.colors = c;
            }
        } else if removed_one(&before.colors, &self.colors) {
            // A colour taken out of the list: keep the rest as they are.
            if self.scheme.colors(self.colors.len()).as_ref() != Some(&self.colors) {
                self.scheme = Scheme::Custom;
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

/// How text in the map, path bar and tips looks: font, size and date formats.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Text {
    /// System font family for labels and the path bar; empty for the built-in font.
    pub family: String,
    pub size_format: SizeFormat,
    pub date_format: String,
}

/// Which details a file label or the tooltip shows.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Shown {
    pub name: bool,
    /// Full path on disk.
    pub path: bool,
    pub size: bool,
    pub modified: bool,
    pub created: bool,
}

impl Shown {
    /// (label, field) for each detail, in the order they're shown.
    pub fn fields(&mut self) -> [(&'static str, &mut bool); 5] {
        [
            ("Name", &mut self.name),
            ("Path", &mut self.path),
            ("Size", &mut self.size),
            ("Modified", &mut self.modified),
            ("Created", &mut self.created),
        ]
    }
}

/// The name / size / date tip shown when hovering a box.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Tooltips {
    pub shown: Shown,
    pub font_size: f32,
    pub delay_ms: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PathBar {
    /// The bar's height follows it (see [`Settings::bar_height`]).
    pub font_size: f32,
}

impl Settings {
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let Some(storage) = storage else {
            return Self::default();
        };
        let mut s: Self =
            eframe::get_value(storage, KEY).unwrap_or_else(|| Self::load_old(storage));
        s.upgrade();
        s
    }

    /// Bring settings saved by older versions up to date.
    fn upgrade(&mut self) {
        // Borders used to be a switch; now a width of 0 turns them off.
        if self.tiles.legacy_borders.take() == Some(false) {
            self.tiles.border.width = 0.0;
        }
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

    /// Layout settings; `title_h` is the folder title bar height for the label font (see
    /// [`fonts::title_height`]).
    pub fn layout_params(&self, hide_dotfiles: bool, title_h: f64) -> LayoutParams {
        LayoutParams {
            density: self.labels.density,
            bias: self.layout.bias,
            show_free: self.layout.show_free,
            hide_dotfiles,
            title_h,
        }
    }

    /// Path bar height: [`BAR_H`] at the default font size, in proportion otherwise.
    pub fn bar_height(&self) -> f32 {
        let base = PathBar::default().font_size;
        (self.path_bar.font_size * BAR_H as f32 / base)
            .round()
            .max(BAR_H as f32 / 2.0)
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
            size: self.text.size_format,
            date_format: self.text.date_format.clone(),
        }
    }

    pub fn infotip_delay(&self) -> Duration {
        Duration::from_millis(self.tooltips.delay_ms as u64)
    }

    /// Zoom factor exponent per point of wheel scroll.
    pub fn wheel_zoom(&self) -> f64 {
        WHEEL_ZOOM * self.general.zoom_speed as f64 / 100.0
    }

    /// Label font for the treemap.
    pub fn map_font(&self) -> egui::FontId {
        egui::FontId::new(self.labels.font_size, fonts::map_family())
    }

    /// Label font for the path bar.
    pub fn bar_font(&self) -> egui::FontId {
        egui::FontId::new(self.path_bar.font_size, fonts::map_family())
    }
}

/// Ctrl + / Ctrl - / Ctrl 0 in this viewport: step the UI zoom setting `z` (percent).
pub fn zoom_keys(ctx: &egui::Context, z: &mut u32) {
    use crate::constants::UI_ZOOM;
    use egui::gui_zoom::kb_shortcuts as kb;
    let step = |z: u32, d: i32| (z as i32 + d).clamp(UI_ZOOM.0 as i32, UI_ZOOM.1 as i32) as u32;
    ctx.input_mut(|i| {
        if i.consume_shortcut(&kb::ZOOM_RESET) {
            *z = 100;
        } else if i.consume_shortcut(&kb::ZOOM_IN) || i.consume_shortcut(&kb::ZOOM_IN_SECONDARY) {
            *z = step(*z, 10);
        } else if i.consume_shortcut(&kb::ZOOM_OUT) {
            *z = step(*z, -10);
        }
    });
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
    fn borders_switch_becomes_width() {
        let mut m = Mem::default();
        // Borders switched off in an older version: width 0, and the switch is gone.
        m.set_string(
            KEY,
            "(tiles: (borders: false, border: (color: None, width: 2.0)))".into(),
        );
        let s = Settings::load(Some(&m));
        assert_eq!(s.tiles.border.width, 0.0);
        assert_eq!(s.tiles.legacy_borders, None);
        // Switched on: the width stays.
        m.set_string(
            KEY,
            "(tiles: (borders: true, border: (color: None, width: 2.0)))".into(),
        );
        assert_eq!(Settings::load(Some(&m)).tiles.border.width, 2.0);
        // Saving doesn't write it back.
        s.save(&mut m);
        assert!(!m.get_string(KEY).unwrap_or_default().contains("borders"));
    }

    #[test]
    fn removing_a_colour_keeps_the_others() {
        let before = Tiles::default();
        let mut t = before.clone();
        t.colors.remove(0);
        t.reconcile(&before);
        assert_eq!(t.colors, before.colors[1..]);
        assert_eq!(t.scheme, Scheme::Custom);
        // The last one gone is still the scheme's own list.
        let mut t = before.clone();
        t.colors.pop();
        t.reconcile(&before);
        assert_eq!(t.scheme, Scheme::Classic);
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
