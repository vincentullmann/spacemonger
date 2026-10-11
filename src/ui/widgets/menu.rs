//! Menu entries for commands, shared by the main menu and the map's right-click menu.

use eframe::egui::{Button, RichText, Ui};
use egui_phosphor::regular as icon;

use super::CommandState;
use crate::core::actions::Action;
use crate::ui::keymap::{shortcut_text, Command, Keymap};

/// Builds a menu of commands, remembering the one clicked.
pub struct CommandMenu<'a> {
    keys: &'a Keymap,
    st: &'a CommandState,
    /// Whether menu entries close the menu themselves (a context menu needs it).
    close_on_click: bool,
    pub clicked: Option<Action>,
}

impl<'a> CommandMenu<'a> {
    /// A menu for `keys` and the app state `st`.
    pub fn new(keys: &'a Keymap, st: &'a CommandState, close_on_click: bool) -> Self {
        Self {
            keys,
            st,
            close_on_click,
            clicked: None,
        }
    }

    /// One entry: the command's icon, name and first shortcut, enabled as the app state allows.
    pub fn item(&mut self, ui: &mut Ui, cmd: Command) {
        let (glyph, label) = self.look(cmd);
        let shortcut = self
            .keys
            .for_command(cmd)
            .next()
            .map(|(_, s)| shortcut_text(s))
            .unwrap_or_default();
        let b = Button::new((RichText::new(glyph), label)).shortcut_text(shortcut);
        if ui.add_enabled(self.st.enabled(cmd), b).clicked() {
            self.clicked = Some(cmd.action(false));
            if self.close_on_click {
                ui.close();
            }
        }
    }

    /// Several entries in a row.
    pub fn items(&mut self, ui: &mut Ui, cmds: &[Command]) {
        for &cmd in cmds {
            self.item(ui, cmd);
        }
    }

    /// Icon and name, for the commands whose look follows the app state.
    fn look(&self, cmd: Command) -> (&'static str, String) {
        let st = self.st;
        match cmd {
            Command::ToggleFree if st.show_free => (icon::CHECK_SQUARE, cmd.menu_label().into()),
            Command::UnhideAll if st.hidden > 0 => {
                (cmd.icon(), format!("{} ({})", cmd.menu_label(), st.hidden))
            }
            _ => (cmd.icon(), cmd.menu_label().into()),
        }
    }
}
