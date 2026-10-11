//! The commands keys and menus run.

use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoStaticStr};

use crate::core::actions::Action;
use crate::core::layout::Dir;
use crate::core::selection::Nav;

/// Something a key can be bound to.
#[derive(
    Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash, EnumIter, IntoStaticStr,
)]
pub enum Command {
    #[strum(serialize = "Open drive / folder")]
    Open,
    #[strum(serialize = "Rescan")]
    Reload,
    #[strum(serialize = "Settings")]
    Settings,
    #[strum(serialize = "Zoom full")]
    ZoomFull,
    #[strum(serialize = "Zoom in")]
    ZoomIn,
    #[strum(serialize = "Zoom out")]
    ZoomOut,
    #[strum(serialize = "Frame selection")]
    Frame,
    #[strum(serialize = "Select up (Shift/Ctrl extends)")]
    Up,
    #[strum(serialize = "Select down (Shift/Ctrl extends)")]
    Down,
    #[strum(serialize = "Select left (Shift/Ctrl extends)")]
    Left,
    #[strum(serialize = "Select right (Shift/Ctrl extends)")]
    Right,
    #[strum(serialize = "Select parent")]
    Parent,
    #[strum(serialize = "Select first child")]
    FirstChild,
    #[strum(serialize = "Clear selection")]
    ClearSelection,
    #[strum(serialize = "Run / open")]
    RunOpen,
    #[strum(serialize = "Delete")]
    Delete,
    #[strum(serialize = "Hide")]
    Hide,
    #[strum(serialize = "Unhide all")]
    UnhideAll,
    #[strum(serialize = "Toggle free space")]
    ToggleFree,
    #[strum(serialize = "Toggle dark mode")]
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

    /// Name on the Hotkeys page (the `strum` serialization above).
    pub fn label(self) -> &'static str {
        self.into()
    }

    /// Name in menus and button tips.
    pub fn menu_label(self) -> &'static str {
        match self {
            Command::Open => "Open…",
            Command::Reload => "Reload",
            Command::Settings => "Settings…",
            Command::ZoomFull => "Zoom to fit",
            Command::ZoomIn => "Zoom in",
            Command::ZoomOut => "Zoom out",
            Command::RunOpen => "Run / open",
            Command::Hide => "Hide",
            Command::UnhideAll => "Unhide all",
            Command::ToggleFree => "Show free space",
            other => other.label(),
        }
    }

    /// Icon in menus and on the title bar.
    pub fn icon(self) -> &'static str {
        match self {
            Command::Open => icon::FOLDER_OPEN,
            Command::Reload => icon::ARROW_CLOCKWISE,
            Command::Settings => icon::GEAR_SIX,
            Command::ZoomFull => icon::CORNERS_OUT,
            Command::ZoomIn => icon::MAGNIFYING_GLASS_PLUS,
            Command::ZoomOut => icon::MAGNIFYING_GLASS_MINUS,
            Command::Frame => icon::FRAME_CORNERS,
            Command::RunOpen => icon::ARROW_SQUARE_OUT,
            Command::Delete => icon::TRASH,
            Command::Hide => icon::EYE_SLASH,
            Command::UnhideAll => icon::EYE,
            Command::ToggleFree => icon::SQUARE,
            Command::ToggleDark => icon::MOON,
            _ => "",
        }
    }

    /// Arrow-style moves: Shift or Ctrl on top of the binding extends the selection.
    pub(super) fn direction(self) -> Option<Dir> {
        match self {
            Command::Up => Some(Dir::Up),
            Command::Down => Some(Dir::Down),
            Command::Left => Some(Dir::Left),
            Command::Right => Some(Dir::Right),
            _ => None,
        }
    }

    /// The action this command runs; `extend` turns an arrow move into extending the
    /// selection.
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
