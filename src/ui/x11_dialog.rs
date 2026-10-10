//! Make the settings window a dialog of the main window on X11: transient for it (so the
//! window manager keeps it in front of the main window and moves it along with it) and
//! left out of the taskbar and pager.
//!
//! egui creates the settings viewport itself and doesn't hand out its window handle, so we
//! find it among this process's top-level windows by title once it's mapped.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// The main window, once known.
static MAIN: AtomicU32 = AtomicU32::new(0);

fn conn() -> Option<&'static (RustConnection, usize)> {
    static CONN: OnceLock<Option<(RustConnection, usize)>> = OnceLock::new();
    CONN.get_or_init(|| x11rb::connect(None).ok()).as_ref()
}

/// Note the main window (call every frame; it only does work once).
pub fn remember_main(frame: &eframe::Frame) {
    if MAIN.load(Ordering::Relaxed) != 0 {
        return;
    }
    if let Ok(h) = frame.window_handle() {
        match h.as_raw() {
            RawWindowHandle::Xlib(w) => MAIN.store(w.window as u32, Ordering::Relaxed),
            RawWindowHandle::Xcb(w) => MAIN.store(w.window.get(), Ordering::Relaxed),
            _ => {}
        }
    }
}

/// Find this process's window titled `title` and make it a dialog of the main window.
/// Returns `true` once done; call again on later frames until it is (the window may not be
/// mapped yet). Not on X11: `true` at once, nothing to do.
pub fn attach(title: &str) -> bool {
    let main = MAIN.load(Ordering::Relaxed);
    let Some((conn, screen)) = conn() else {
        return true;
    };
    if main == 0 {
        return false;
    }
    let root = conn.setup().roots[*screen].root;
    match find(conn, root, title) {
        Some(w) => {
            let _ = make_dialog(conn, root, w, main);
            true
        }
        None => false,
    }
}

fn atom(conn: &RustConnection, name: &str) -> Option<u32> {
    Some(
        conn.intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom,
    )
}

/// Top-level windows: the window manager's client list, or the root's children without one.
fn find(conn: &RustConnection, root: Window, title: &str) -> Option<Window> {
    let client_list = atom(conn, "_NET_CLIENT_LIST")?;
    let mut candidates: Vec<Window> = conn
        .get_property(false, root, client_list, AtomEnum::WINDOW, 0, 4096)
        .ok()?
        .reply()
        .ok()
        .and_then(|r| r.value32().map(|v| v.collect()))
        .unwrap_or_default();
    if let Ok(Ok(tree)) = conn.query_tree(root).map(|c| c.reply()) {
        candidates.extend(tree.children);
    }
    let (pid_atom, name_atom, utf8) = (
        atom(conn, "_NET_WM_PID")?,
        atom(conn, "_NET_WM_NAME")?,
        atom(conn, "UTF8_STRING")?,
    );
    let pid = std::process::id();
    candidates.into_iter().find(|&w| {
        let ours = conn
            .get_property(false, w, pid_atom, AtomEnum::CARDINAL, 0, 1)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().and_then(|mut v| v.next()))
            == Some(pid);
        let named = conn
            .get_property(false, w, name_atom, utf8, 0, 256)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|r| r.value == title.as_bytes());
        ours && named
    })
}

fn make_dialog(conn: &RustConnection, root: Window, w: Window, main: Window) -> Option<()> {
    conn.change_property32(
        PropMode::REPLACE,
        w,
        AtomEnum::WM_TRANSIENT_FOR,
        AtomEnum::WINDOW,
        &[main],
    )
    .ok()?;
    let ty = atom(conn, "_NET_WM_WINDOW_TYPE")?;
    let dialog = atom(conn, "_NET_WM_WINDOW_TYPE_DIALOG")?;
    conn.change_property32(PropMode::REPLACE, w, ty, AtomEnum::ATOM, &[dialog])
        .ok()?;
    // _NET_WM_STATE on a mapped window is changed by asking the window manager.
    let state = atom(conn, "_NET_WM_STATE")?;
    let skip_taskbar = atom(conn, "_NET_WM_STATE_SKIP_TASKBAR")?;
    let skip_pager = atom(conn, "_NET_WM_STATE_SKIP_PAGER")?;
    const ADD: u32 = 1;
    let msg = ClientMessageEvent::new(32, w, state, [ADD, skip_taskbar, skip_pager, 1, 0]);
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        msg,
    )
    .ok()?;
    conn.flush().ok()
}
