//! The settings groups, one struct per settings page (or part of one). Defaults are in
//! `defaults.rs`.

use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoStaticStr};

use crate::ui::keymap::Keymap;
use crate::utils::format::SizeFormat;

pub use super::tiles::Tiles;

////////////////////////////////////////////////////////////////////////////////
// region:Settings

/// Everything kept between runs.
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

////////////////////////////////////////////////////////////////////////////////
// region:General

/// Light or dark window colours.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, EnumIter, IntoStaticStr,
)]
pub enum Theme {
    #[default]
    Light,
    Dark,
    /// Follow the desktop's light / dark preference.
    #[strum(serialize = "Follow system")]
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

/// Window-wide behaviour: theme, zoom, animation, confirmations.
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

////////////////////////////////////////////////////////////////////////////////
// region:Display

/// How the treemap is split.
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

/// What the scanner reads.
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

/// The breadcrumb bar above the map.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PathBar {
    /// The bar's height follows it (see [`Settings::bar_height`]).
    pub font_size: f32,
}
