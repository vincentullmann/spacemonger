//! X11 extras for our undecorated windows, beyond what winit offers: smooth resizing,
//! window-manager shadow and window menu, and the settings window as a dialog of the main one.
//!
//! Everything goes over one connection of our own ([`conn`]), kept open for the life of the
//! app: the shadow pixmaps belong to it and would be freed with it. On Wayland, or without
//! an X server, every call here quietly does nothing.

mod dialog;
mod shadow;
mod sync;
mod wm;

pub use dialog::{attach_dialog, remember_main, Attach};
pub use sync::{acknowledge, frame_drawn, install_sync, set_background};
pub use wm::{add_shadow, composited, install_wm, set_shadow, show_window_menu};

use std::error::Error;
use std::sync::OnceLock;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::protocol::xproto::{ConnectionExt as _, Window};
use x11rb::rust_connection::RustConnection;

/// Errors from setting things up: only ever logged.
type XResult<T> = Result<T, Box<dyn Error>>;

/// The app's own X connection and its screen number; `None` without an X server.
fn conn() -> Option<&'static (RustConnection, usize)> {
    static CONN: OnceLock<Option<(RustConnection, usize)>> = OnceLock::new();
    CONN.get_or_init(|| x11rb::connect(None).ok()).as_ref()
}

/// The root window of our screen.
fn root(conn: &RustConnection, screen: usize) -> Window {
    use x11rb::connection::Connection as _;
    conn.setup().roots[screen].root
}

/// Look up (or create) the atom called `name`.
fn atom(conn: &RustConnection, name: &str) -> XResult<u32> {
    Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
}

/// What a frame's native window handle tells us.
enum Handle {
    /// Not created yet: try again next frame.
    Pending,
    /// An X11 window.
    X11(Window),
    /// Some other windowing system (Wayland).
    Other,
}

/// The X11 window behind eframe's window, if it is one.
fn window_of(frame: &eframe::Frame) -> Handle {
    let Ok(h) = frame.window_handle() else {
        return Handle::Pending;
    };
    match h.as_raw() {
        RawWindowHandle::Xlib(w) => Handle::X11(w.window as Window),
        RawWindowHandle::Xcb(w) => Handle::X11(w.window.get()),
        _ => Handle::Other,
    }
}
