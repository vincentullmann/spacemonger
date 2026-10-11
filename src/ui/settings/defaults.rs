//! Default values for the settings. Edit them here.

use super::{General, Labels, Layout, Line, PathBar, Scan, Shown, Text, Theme, Tiles, Tooltips};
use crate::ui::palette::Scheme;
use crate::utils::format::{SizeFormat, DEFAULT_DATE_FORMAT};


////////////////////////////////////////////////////////////////////////////////
// region:General


impl Default for General {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            anim_ms: 250,
            zoom_speed: 100,
            frame_fill: 90,
            confirm_delete: true,
        }
    }
}


impl Default for Layout {
    fn default() -> Self {
        Self {
            bias: 0,
            show_free: true,
        }
    }
}


////////////////////////////////////////////////////////////////////////////////
// region:Scan


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


////////////////////////////////////////////////////////////////////////////////
// region:Display


impl Default for Text {
    fn default() -> Self {
        Self {
            family: String::new(),
            size_format: SizeFormat::Decimal,
            date_format: DEFAULT_DATE_FORMAT.to_string(),
        }
    }
}


impl Default for Tiles {
    fn default() -> Self {
        Self {
            scheme: Scheme::Classic,

            colors: Scheme::Classic.colors(8).unwrap_or_default(),

            borders: true,
            border: Line {
                color: None,
                width: 1.0,
            },
            hover_border: Line {
                color: None,
                width: 2.0,
            },
            selection_color: None,
            gap: 0,
            hover: 20,
        }
    }
}


// aka. tile labels
impl Default for Labels {
    fn default() -> Self {
        Self {
            font_size: 10.0,
            shadow: false,
            density: 0,

            shown: Shown {
                name: true,
                size: true,
                ..Default::default()
            },
        }
    }
}


impl Default for PathBar {
    fn default() -> Self {
        Self { font_size: 10.0 }
    }
}


impl Default for Tooltips {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            delay_ms: 250,
            shown: Shown {
                name: true,
                size: true,
                created: false,
                modified: true,
                ..Default::default()
            },
        }
    }
}
