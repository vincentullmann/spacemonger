//! Shortcut text and comparison.

use eframe::egui::{Event, InputState, Key, KeyboardShortcut, ModifierNames, Modifiers};

/// (alt, ctrl/cmd, shift): what a shortcut's modifiers mean, platform aside.
pub(super) fn norm(m: Modifiers) -> (bool, bool, bool) {
    (m.alt, m.ctrl || m.command || m.mac_cmd, m.shift)
}

/// "Ctrl+Shift+H", "Ctrl+,": punctuation as its symbol, other keys by name.
pub fn shortcut_text(s: &KeyboardShortcut) -> String {
    let names = ModifierNames::NAMES;
    let mut text = names.format(&s.modifiers, cfg!(target_os = "macos"));
    if !text.is_empty() {
        text += names.concat;
    }
    // egui's symbols for the arrows are pictures; "Up" reads better there.
    let key = s.logical_key;
    let arrow = matches!(
        key,
        Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight
    );
    text += if arrow {
        key.name()
    } else {
        key.symbol_or_name()
    };
    text
}

/// Same key and modifiers, however the modifiers were recorded.
pub fn same_shortcut(a: &KeyboardShortcut, b: &KeyboardShortcut) -> bool {
    a.logical_key == b.logical_key && norm(a.modifiers) == norm(b.modifiers)
}

/// A key pressed this frame while waiting for a new shortcut (consumed): `Some(None)` for
/// Esc (cancel), `Some(Some(shortcut))` for anything else, `None` while still waiting.
pub fn capture(i: &mut InputState) -> Option<Option<KeyboardShortcut>> {
    let found = i.events.iter().find_map(|e| match e {
        Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } => Some((*key, *modifiers)),
        _ => None,
    })?;
    i.events.retain(|e| !matches!(e, Event::Key { .. }));
    Some(match found {
        (Key::Escape, m) if m.is_none() => None,
        (key, m) => Some(KeyboardShortcut::new(m, key)),
    })
}
