//! Keyboard shortcuts: a user-editable table from shortcut to command.

use crate::core::actions::Action;
use crate::core::layout::Dir;
use crate::core::selection::Nav;
use eframe::egui::{Event, InputState, Key, KeyboardShortcut, ModifierNames, Modifiers};
use serde::{Deserialize, Serialize};

/// Something a key can be bound to.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Command {
    Open,
    Reload,
    Settings,
    ZoomFull,
    ZoomIn,
    ZoomOut,
    Frame,
    Up,
    Down,
    Left,
    Right,
    Parent,
    FirstChild,
    ClearSelection,
    RunOpen,
    Delete,
    Hide,
    UnhideAll,
    ToggleFree,
    ToggleDark,
}

impl Command {
    /// Every command, by group, in the order the Keys tab lists them.
    pub const GROUPS: [(&'static str, &'static [Command]); 4] = [
        ("Scanning", &[Command::Open, Command::Reload]),
        (
            "Navigation",
            &[
                Command::ZoomFull,
                Command::ZoomIn,
                Command::ZoomOut,
                Command::Frame,
                Command::Up,
                Command::Down,
                Command::Left,
                Command::Right,
                Command::Parent,
                Command::FirstChild,
            ],
        ),
        (
            "Selection & actions",
            &[
                Command::ClearSelection,
                Command::RunOpen,
                Command::Delete,
                Command::Hide,
                Command::UnhideAll,
            ],
        ),
        (
            "Display",
            &[Command::ToggleFree, Command::ToggleDark, Command::Settings],
        ),
    ];

    pub fn label(self) -> &'static str {
        match self {
            Command::Open => "Open drive / folder",
            Command::Reload => "Rescan",
            Command::Settings => "Settings",
            Command::ZoomFull => "Zoom full",
            Command::ZoomIn => "Zoom in",
            Command::ZoomOut => "Zoom out",
            Command::Frame => "Frame selection",
            Command::Up => "Select up (Shift/Ctrl extends)",
            Command::Down => "Select down (Shift/Ctrl extends)",
            Command::Left => "Select left (Shift/Ctrl extends)",
            Command::Right => "Select right (Shift/Ctrl extends)",
            Command::Parent => "Select parent",
            Command::FirstChild => "Select first child",
            Command::ClearSelection => "Clear selection",
            Command::RunOpen => "Run / open",
            Command::Delete => "Delete",
            Command::Hide => "Hide",
            Command::UnhideAll => "Unhide all",
            Command::ToggleFree => "Toggle free space",
            Command::ToggleDark => "Toggle dark mode",
        }
    }

    /// Arrow-style moves: Shift or Ctrl on top of the binding extends the selection.
    fn direction(self) -> Option<Dir> {
        match self {
            Command::Up => Some(Dir::Up),
            Command::Down => Some(Dir::Down),
            Command::Left => Some(Dir::Left),
            Command::Right => Some(Dir::Right),
            _ => None,
        }
    }

    pub fn action(self, extend: bool) -> Action {
        if let Some(d) = self.direction() {
            return Action::Nav(if extend {
                Nav::Extend(d)
            } else {
                Nav::Sibling(d)
            });
        }
        match self {
            Command::Open => Action::Open,
            Command::Reload => Action::Reload,
            Command::Settings => Action::Settings,
            Command::ZoomFull => Action::ZoomFull,
            Command::ZoomIn => Action::ZoomIn,
            Command::ZoomOut => Action::ZoomOut,
            Command::Frame => Action::Frame,
            Command::Parent => Action::Nav(Nav::Parent),
            Command::FirstChild => Action::Nav(Nav::FirstChild),
            Command::ClearSelection => Action::ClearSelection,
            Command::RunOpen => Action::RunOpen,
            Command::Delete => Action::Delete,
            Command::Hide => Action::Hide,
            Command::UnhideAll => Action::UnhideAll,
            Command::ToggleFree => Action::ToggleFree,
            Command::ToggleDark => Action::ToggleDark,
            Command::Up | Command::Down | Command::Left | Command::Right => unreachable!(),
        }
    }
}

/// (alt, ctrl/cmd, shift): what a shortcut's modifiers mean, platform aside.
fn norm(m: Modifiers) -> (bool, bool, bool) {
    (m.alt, m.ctrl || m.command || m.mac_cmd, m.shift)
}

/// "Ctrl+Shift+H"
pub fn shortcut_text(s: &KeyboardShortcut) -> String {
    s.format(&ModifierNames::NAMES, cfg!(target_os = "macos"))
}

/// Same key and modifiers, however the modifiers were recorded.
pub fn same_shortcut(a: &KeyboardShortcut, b: &KeyboardShortcut) -> bool {
    a.logical_key == b.logical_key && norm(a.modifiers) == norm(b.modifiers)
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Keymap {
    pub binds: Vec<(Command, KeyboardShortcut)>,
}

impl Default for Keymap {
    fn default() -> Self {
        let none = Modifiers::NONE;
        let k = |m, key| KeyboardShortcut::new(m, key);
        Self {
            binds: vec![
                (Command::Open, k(Modifiers::COMMAND, Key::O)),
                (Command::Reload, k(none, Key::F5)),
                (Command::Settings, k(Modifiers::COMMAND, Key::Comma)),
                (Command::ZoomIn, k(none, Key::Enter)),
                (Command::ZoomOut, k(none, Key::Backspace)),
                (Command::Frame, k(none, Key::F)),
                (Command::Up, k(none, Key::ArrowUp)),
                (Command::Down, k(none, Key::ArrowDown)),
                (Command::Left, k(none, Key::ArrowLeft)),
                (Command::Right, k(none, Key::ArrowRight)),
                (Command::Parent, k(Modifiers::ALT, Key::ArrowUp)),
                (Command::FirstChild, k(Modifiers::ALT, Key::ArrowDown)),
                (Command::ClearSelection, k(none, Key::Escape)),
                (Command::Delete, k(none, Key::Delete)),
                (Command::Hide, k(none, Key::H)),
                (Command::UnhideAll, k(Modifiers::SHIFT, Key::H)),
            ],
        }
    }
}

impl Keymap {
    /// Shortcuts bound to `cmd`, with their index in `binds`.
    pub fn for_command(&self, cmd: Command) -> impl Iterator<Item = (usize, &KeyboardShortcut)> {
        self.binds
            .iter()
            .enumerate()
            .filter(move |(_, (c, _))| *c == cmd)
            .map(|(i, (_, s))| (i, s))
    }

    /// Default bindings for one command.
    pub fn defaults_for(cmd: Command) -> Vec<KeyboardShortcut> {
        Self::default()
            .binds
            .into_iter()
            .filter(|(c, _)| *c == cmd)
            .map(|(_, s)| s)
            .collect()
    }

    /// Put `cmd`'s default bindings back.
    pub fn reset(&mut self, cmd: Command) {
        self.binds.retain(|(c, _)| *c != cmd);
        self.binds
            .extend(Self::defaults_for(cmd).into_iter().map(|s| (cmd, s)));
    }

    /// Commands bound to the same shortcut as `binds[i]`, other than its own.
    pub fn conflicts(&self, i: usize) -> Vec<Command> {
        let (cmd, s) = self.binds[i];
        self.binds
            .iter()
            .filter(|(c, o)| *c != cmd && same_shortcut(o, &s))
            .map(|(c, _)| *c)
            .collect()
    }

    /// The command for one key press: an exact match first, then an arrow-style move with
    /// Shift / Ctrl added on top.
    fn lookup(&self, key: Key, mods: Modifiers) -> Option<Action> {
        let pressed = norm(mods);
        if let Some((c, _)) = self
            .binds
            .iter()
            .find(|(_, s)| s.logical_key == key && norm(s.modifiers) == pressed)
        {
            return Some(c.action(false));
        }
        let (alt, ctrl, shift) = pressed;
        if !(ctrl || shift) {
            return None;
        }
        self.binds
            .iter()
            .find(|(c, s)| {
                c.direction().is_some()
                    && s.logical_key == key
                    && norm(s.modifiers) == (alt, false, false)
            })
            .map(|(c, _)| c.action(true))
    }

    /// The action for this frame's key presses, if any.
    pub fn action_for_keys(&self, i: &InputState) -> Option<Action> {
        i.events.iter().find_map(|e| match e {
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => self.lookup(*key, *modifiers),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_matches_like_before() {
        let m = Keymap::default();
        let shift = Modifiers::SHIFT;
        assert_eq!(m.lookup(Key::H, Modifiers::NONE), Some(Action::Hide));
        assert_eq!(m.lookup(Key::H, shift), Some(Action::UnhideAll));
        assert_eq!(m.lookup(Key::F, Modifiers::CTRL), None);
        assert_eq!(
            m.lookup(Key::ArrowUp, Modifiers::NONE),
            Some(Action::Nav(Nav::Sibling(Dir::Up)))
        );
        assert_eq!(
            m.lookup(Key::ArrowUp, shift),
            Some(Action::Nav(Nav::Extend(Dir::Up)))
        );
        assert_eq!(
            m.lookup(Key::ArrowLeft, Modifiers::CTRL),
            Some(Action::Nav(Nav::Extend(Dir::Left)))
        );
        assert_eq!(
            m.lookup(Key::ArrowUp, Modifiers::ALT),
            Some(Action::Nav(Nav::Parent))
        );
        assert_eq!(
            m.lookup(Key::Comma, Modifiers::CTRL),
            Some(Action::Settings)
        );
    }

    #[test]
    fn groups_list_every_command_once() {
        let all: Vec<Command> = Command::GROUPS
            .iter()
            .flat_map(|(_, c)| c.iter().copied())
            .collect();
        assert_eq!(all.len(), 20);
        assert!(all.iter().enumerate().all(|(i, c)| !all[..i].contains(c)));
        // Every default binding's command is listed.
        assert!(Keymap::default().binds.iter().all(|(c, _)| all.contains(c)));
    }

    #[test]
    fn reset_and_conflicts() {
        let mut m = Keymap::default();
        m.binds.retain(|(c, _)| *c != Command::Hide);
        m.binds.push((
            Command::Frame,
            KeyboardShortcut::new(Modifiers::NONE, Key::Delete),
        ));
        let i = m.binds.len() - 1;
        assert_eq!(m.conflicts(i), vec![Command::Delete]);
        m.reset(Command::Hide);
        assert_eq!(m.lookup(Key::H, Modifiers::NONE), Some(Action::Hide));
    }
}
