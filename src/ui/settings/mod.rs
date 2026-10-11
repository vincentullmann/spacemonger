//! Settings kept between runs (eframe storage) and edited live in the settings window.

mod defaults;
mod form;
mod hotkeys;
mod pages;
mod sections;
mod storage;
mod tiles;
mod window;

pub use sections::{
    General, Labels, Layout, Line, PathBar, Scan, Settings, Shown, Text, Theme, Tiles, Tooltips,
};
pub use window::SettingsWindow;

use std::time::Duration;

use eframe::egui;

use crate::constants::{BAR_H, WHEEL_ZOOM};
use crate::core::camera::CameraParams;
use crate::core::fs::ScanOptions;
use crate::core::layout::LayoutParams;
use crate::ui::fonts;
use crate::utils::format::FormatOptions;

/// What the rest of the app takes from the settings.
impl Settings {
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
