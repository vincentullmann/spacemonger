//! Settings kept between runs (eframe storage).

use serde::{Deserialize, Serialize};

const KEY: &str = "settings";
/// Keys used before `Settings` existed.
const LEGACY_KEYS: [&str; 2] = ["dark", "show_free"];

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub dark: bool,
    pub show_free: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { dark: false, show_free: true }
    }
}

impl Settings {
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let Some(storage) = storage else { return Self::default() };
        eframe::get_value(storage, KEY).unwrap_or_else(|| Self::load_legacy(storage))
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, KEY, self);
        for k in LEGACY_KEYS {
            storage.remove_string(k);
        }
    }

    /// Settings saved by older versions, one string per key.
    fn load_legacy(storage: &dyn eframe::Storage) -> Self {
        let get = |k: &str| storage.get_string(k);
        Self {
            dark: get("dark").is_some_and(|v| v == "true"),
            show_free: get("show_free").is_none_or(|v| v == "true"),
        }
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

    #[test]
    fn round_trip_and_legacy() {
        assert_eq!(Settings::load(None), Settings::default());
        let mut m = Mem::default();
        assert_eq!(Settings::load(Some(&m)), Settings::default());
        // Old string keys still load.
        m.set_string("dark", "true".into());
        m.set_string("show_free", "false".into());
        assert_eq!(Settings::load(Some(&m)), Settings { dark: true, show_free: false });
        // New key wins once saved.
        Settings { dark: false, show_free: false }.save(&mut m);
        assert_eq!(Settings::load(Some(&m)), Settings { dark: false, show_free: false });
        assert!(m.get_string("dark").is_none());
    }
}
