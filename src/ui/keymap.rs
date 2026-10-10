//! Keyboard shortcuts.

use crate::core::actions::Action;
use crate::core::layout::Dir;
use crate::core::selection::Nav;
use eframe::egui::{InputState, Key};

/// The action for this frame's key presses, if any.
pub fn action_for_keys(i: &InputState) -> Option<Action> {
    let extend = i.modifiers.shift || i.modifiers.command;
    let arrow = |d| {
        Action::Nav(if extend {
            Nav::Extend(d)
        } else {
            Nav::Sibling(d)
        })
    };
    if i.key_pressed(Key::Backspace) {
        Some(Action::ZoomOut)
    } else if i.key_pressed(Key::Enter) {
        Some(Action::ZoomIn)
    } else if i.key_pressed(Key::Delete) {
        Some(Action::Delete)
    } else if i.key_pressed(Key::H) && i.modifiers.shift {
        Some(Action::UnhideAll)
    } else if i.key_pressed(Key::H) {
        Some(Action::Hide)
    } else if i.key_pressed(Key::F5) {
        Some(Action::Reload)
    } else if i.key_pressed(Key::F) && !i.modifiers.any() {
        Some(Action::Frame)
    } else if i.key_pressed(Key::ArrowUp) && i.modifiers.alt {
        Some(Action::Nav(Nav::Parent))
    } else if i.key_pressed(Key::ArrowDown) && i.modifiers.alt {
        Some(Action::Nav(Nav::FirstChild))
    } else if i.key_pressed(Key::ArrowUp) {
        Some(arrow(Dir::Up))
    } else if i.key_pressed(Key::ArrowDown) {
        Some(arrow(Dir::Down))
    } else if i.key_pressed(Key::ArrowLeft) {
        Some(arrow(Dir::Left))
    } else if i.key_pressed(Key::ArrowRight) {
        Some(arrow(Dir::Right))
    } else if i.key_pressed(Key::Escape) {
        Some(Action::ClearSelection)
    } else {
        None
    }
}
