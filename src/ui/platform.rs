//! Platform hooks the windows call into, so callers need no `#[cfg]` of their own. On Linux
//! they go to [`crate::ui::x11`] (and do nothing on Wayland); elsewhere they do nothing.

use eframe::egui::{Color32, Context, Pos2};

/// Start of the main window's frame: set up the X11 extras once the window exists, and keep
/// the uncovered-area colour and the shadow (none while maximised) up to date.
pub fn main_frame_start(frame: &eframe::Frame, ctx: &Context, background: Color32) {
    #[cfg(target_os = "linux")]
    {
        use crate::ui::x11;
        x11::install_sync(frame, ctx, background);
        x11::remember_main(frame);
        x11::set_background(background);
        x11::install_wm(frame);
        let full = ctx.input(|i| {
            let v = i.viewport();
            v.maximized.unwrap_or(false) || v.fullscreen.unwrap_or(false)
        });
        x11::set_shadow(!full);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (frame, ctx, background);
}

/// End of the main window's frame: it shows the window's current size.
pub fn main_frame_drawn(ctx: &Context) {
    #[cfg(target_os = "linux")]
    crate::ui::x11::frame_drawn(ctx);
    #[cfg(not(target_os = "linux"))]
    let _ = ctx;
}

/// Before each frame: the previous one is on screen.
pub fn frame_presented() {
    #[cfg(target_os = "linux")]
    crate::ui::x11::acknowledge();
}

/// Whether windows can have see-through (rounded) corners.
pub fn composited() -> bool {
    #[cfg(target_os = "linux")]
    return crate::ui::x11::composited();
    #[cfg(not(target_os = "linux"))]
    true
}

/// The window manager's window menu at `px` (window pixels), where it offers one.
pub fn show_window_menu(px: Pos2) {
    #[cfg(target_os = "linux")]
    crate::ui::x11::show_window_menu(px);
    #[cfg(not(target_os = "linux"))]
    let _ = px;
}

/// Make our window titled `title` a dialog of the main window (with its shadow). `false`
/// while the window isn't there yet: call again next frame.
pub fn attach_dialog(title: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        use crate::ui::x11::{self, Attach};
        match x11::attach_dialog(title) {
            Attach::Pending => false,
            Attach::Done(window) => {
                if let Some(w) = window {
                    x11::add_shadow(w);
                }
                true
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = title;
        true
    }
}
