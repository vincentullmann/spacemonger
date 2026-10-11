//! Moving the undecorated windows: drag empty space to move, handed to the window manager,
//! and making up for the button release the window manager then swallows.

use std::collections::HashMap;

use eframe::egui::{
    self, Event, Modifiers, PointerButton, Pos2, RawInput, Ui, ViewportCommand, ViewportId,
};

/// Pointer travel (points) with the button down before a press moves the window.
const DRAG_THRESHOLD: f32 = 3.0;

/// Drag empty bar space to move the window, double-click it to toggle maximised.
pub fn move_or_maximize(
    ui: &Ui,
    bg: &egui::Response,
    empty: impl Fn(egui::Pos2) -> bool,
    maximized: bool,
) {
    if bg.double_clicked() {
        ui.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        return;
    }
    move_on_drag(ui, bg, empty);
}

/// Drag `bg` (where `empty` says it isn't covered) to move the window.
///
/// The move starts once the pointer has moved a few pixels with the button down, tracked
/// here from the raw pointer rather than egui's drag state, so it also works on the click
/// that focuses an inactive window. Then the window manager takes over (see `wm_grab`).
pub fn move_on_drag(ui: &Ui, bg: &egui::Response, empty: impl Fn(egui::Pos2) -> bool) {
    let (pressed, down, pos) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.interact_pos(),
        )
    });
    let id = bg.id.with("press");
    if pressed {
        let start = pos.filter(|&p| bg.contains_pointer() && empty(p));
        ui.data_mut(|d| d.insert_temp(id, start));
        return;
    }
    let start: Option<egui::Pos2> = ui.data(|d| d.get_temp(id)).flatten();
    let Some(start) = start else { return };
    if !down {
        ui.data_mut(|d| d.remove::<Option<egui::Pos2>>(id));
    } else if pos.is_some_and(|p| p.distance(start) > DRAG_THRESHOLD) {
        ui.data_mut(|d| d.remove::<Option<egui::Pos2>>(id));
        wm_grab(ui.ctx(), ViewportCommand::StartDrag);
    }
}

/// Send a move or resize that the window manager carries out (`StartDrag`, `BeginResize`).
/// The WM takes the pointer, so the button release never reaches us, and egui would think the
/// button is still down: no hover until the next click. [`GrabRelease`] makes it up.
pub fn wm_grab(ctx: &egui::Context, cmd: ViewportCommand) {
    ctx.send_viewport_cmd(cmd);
    let at = ctx.input(|i| (i.pointer.interact_pos(), i.modifiers));
    if let (Some(pos), modifiers) = at {
        let id = ctx.viewport_id();
        ctx.plugin_or_default::<GrabRelease>()
            .lock()
            .pending
            .insert(id, (pos, modifiers));
    }
    ctx.request_repaint();
}

/// Adds the button release a [`wm_grab`] swallows to the next input of that viewport.
/// An egui plugin rather than the app's `raw_input_hook`, which only sees the main window.
#[derive(Default)]
pub struct GrabRelease {
    pending: HashMap<ViewportId, (Pos2, Modifiers)>,
}

impl egui::Plugin for GrabRelease {
    fn debug_name(&self) -> &'static str {
        "GrabRelease"
    }

    fn input_hook(&mut self, _ctx: &egui::Context, raw: &mut RawInput) {
        let Some((pos, modifiers)) = self.pending.remove(&raw.viewport_id) else {
            return;
        };
        let released = raw.events.iter().any(|e| {
            matches!(
                e,
                Event::PointerButton {
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                }
            )
        });
        if !released {
            raw.events.push(Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers,
            });
        }
    }
}
