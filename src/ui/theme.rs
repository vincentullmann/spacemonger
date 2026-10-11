//! Colours for the window chrome: title bar, settings, menus, dialogs and tooltips.
//! The treemap keeps its own palette and does not read this.

use eframe::egui::{self, Color32, Stroke};

/// One set of chrome colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Panels, the title bar, settings, menus and tooltips.
    pub background: Color32,

    /// Body text, buttons and icons.
    pub text_color_main: Color32,

    /// The window title, hints, and other secondary text.
    pub text_color_dimmed: Color32,

    /// Section titles and other strong text.
    pub text_color_header: Color32,

    /// Selection, progress, links and the text cursor.
    pub accent_color: Color32,
}


impl Theme {
    pub const fn light() -> Self {
        Self {
            background: Color32::from_rgb(0xF6, 0xF6, 0xF4),
            text_color_main: Color32::from_rgb(0x1F, 0x1F, 0x1D),
            text_color_dimmed: Color32::from_rgb(0x6B, 0x6B, 0x66),
            text_color_header: Color32::from_rgb(0x0E, 0x0E, 0x0C),
            accent_color: Color32::from_rgb(0, 120, 212),
        }
    }

    pub const fn dark() -> Self {
        Self {
            background: Color32::from_rgb(0x22, 0x23, 0x24),
            text_color_main: Color32::from_rgb(0xE6, 0xE6, 0xE0),
            text_color_dimmed: Color32::from_rgb(0x98, 0x98, 0x92),
            text_color_header: Color32::from_rgb(0xF6, 0xF6, 0xF0),
            accent_color: Color32::from_rgb(0x5A, 0xAB, 0xFF),
        }
    }

    pub const fn of(dark: bool) -> Self {
        if dark {
            Self::dark()
        } else {
            Self::light()
        }
    }

    /// Install both themes. The settings preference chooses which one is active.
    /// Buttons, check boxes and drop-downs show a hand cursor in either.
    pub fn install(ctx: &egui::Context) {
        for (mode, theme) in [
            (egui::Theme::Light, Self::light()),
            (egui::Theme::Dark, Self::dark()),
        ] {
            ctx.style_mut_of(mode, |style| {
                style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
                theme.apply(&mut style.visuals);
            });
        }
    }

    /// Paint egui's panels, text, widgets and popups with these colours.
    pub fn apply(&self, visuals: &mut egui::Visuals) {
        let dark = self.background.intensity() < 0.5;
        visuals.dark_mode = dark;

        visuals.panel_fill = self.background;
        visuals.window_fill = self.background;
        visuals.extreme_bg_color = if dark {
            mix(self.background, Color32::BLACK, 0.45)
        } else {
            Color32::WHITE
        };
        visuals.faint_bg_color = mix(self.background, self.text_color_dimmed, 0.08);
        visuals.code_bg_color = visuals.extreme_bg_color;

        visuals.override_text_color = Some(self.text_color_main);
        visuals.weak_text_color = Some(self.text_color_dimmed);

        let line = Stroke::new(1.0, mix(self.background, self.text_color_dimmed, 0.55));
        visuals.window_stroke = line;
        visuals.widgets.noninteractive.bg_stroke = line;
        visuals.widgets.noninteractive.weak_bg_fill = self.background;
        visuals.widgets.noninteractive.bg_fill = self.background;

        // `strong_text_color()` is the active widget's text colour, which section titles use.
        let widgets = &mut visuals.widgets;
        widgets.noninteractive.fg_stroke.color = self.text_color_main;
        widgets.inactive.fg_stroke.color = self.text_color_main;
        widgets.hovered.fg_stroke.color = self.text_color_main;
        widgets.open.fg_stroke.color = self.text_color_main;
        widgets.active.fg_stroke.color = self.text_color_header;

        let raised = |step: f32| {
            let (to, t) = if dark {
                (Color32::WHITE, step)
            } else {
                (Color32::BLACK, step * 0.65)
            };
            mix(self.background, to, t)
        };
        let fill = |widget: &mut egui::style::WidgetVisuals, color: Color32| {
            widget.weak_bg_fill = color;
            widget.bg_fill = color;
        };
        fill(&mut widgets.inactive, raised(0.12));
        fill(&mut widgets.hovered, raised(0.18));
        fill(&mut widgets.active, raised(0.24));
        fill(&mut widgets.open, raised(0.08));
        widgets.open.bg_fill = self.background;
        widgets.hovered.bg_stroke = Stroke::new(1.0, self.accent_color);

        visuals.hyperlink_color = self.accent_color;
        visuals.selection.bg_fill = self.accent_color;
        visuals.selection.stroke.color = if self.accent_color.intensity() > 0.6 {
            Color32::BLACK
        } else {
            Color32::WHITE
        };
        visuals.text_cursor.stroke.color = self.accent_color;
        // One glyph rasterizer for both themes. Dark mode thickens antialiased edges, which
        // makes the treemap's black labels look like they grew a shadow.
        visuals.text_options.color_transfer_function =
            egui::epaint::FontColorTransferFunction::LIGHT_MODE_DEFAULT;
    }
}

fn mix(from: Color32, to: Color32, t: f32) -> Color32 {
    from.lerp_to_gamma(to, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_is_light_and_dark_is_dark() {
        let light = Theme::light();
        let dark = Theme::dark();
        assert!(light.background.intensity() > 0.5);
        assert!(dark.background.intensity() < 0.5);
        assert!(light.text_color_main.intensity() < dark.text_color_main.intensity());
        assert_eq!(Theme::of(false), light);
        assert_eq!(Theme::of(true), dark);
    }

    #[test]
    fn apply_sets_chrome_colours() {
        let theme = Theme::light();
        let mut visuals = egui::Visuals::dark();
        theme.apply(&mut visuals);
        assert!(!visuals.dark_mode);
        assert_eq!(visuals.panel_fill, theme.background);
        assert_eq!(visuals.window_fill, theme.background);
        assert_eq!(visuals.text_color(), theme.text_color_main);
        assert_eq!(visuals.weak_text_color(), theme.text_color_dimmed);
        assert_eq!(visuals.strong_text_color(), theme.text_color_header);
        assert_eq!(visuals.hyperlink_color, theme.accent_color);
        assert_eq!(visuals.selection.bg_fill, theme.accent_color);
    }
}
