//! Loading and saving the settings (eframe storage), including the keys older versions used.

use serde::Deserialize;

use super::{Settings, Theme};

/// Current storage key.
const KEY: &str = "settings.v2";
/// `{ dark, show_free }`, saved by the first `Settings`.
const KEY_V1: &str = "settings";
/// Keys used before `Settings` existed.
const LEGACY_KEYS: [&str; 2] = ["dark", "show_free"];

impl Settings {
    /// The saved settings, brought up to date; the defaults when there are none.
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
}
