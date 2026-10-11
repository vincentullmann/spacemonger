//! Make the settings window a dialog of the main window on X11: transient for it (so the
//! window manager keeps it in front of the main window and moves it along with it) and
//! left out of the taskbar and pager.
//!
//! egui creates the settings viewport itself and doesn't hand out its window handle, so we
//! find it among this process's top-level windows by title once it's mapped.

use std::sync::atomic::{AtomicU32, Ordering};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::{atom, conn, root, window_of, Handle, XResult};

/// How many windows the property lists are read up to.
const MAX_WINDOWS: u32 = 4096;
/// Longest window title compared, in 32-bit units.
const MAX_TITLE: u32 = 256;
/// `_NET_WM_STATE` client message: add the listed states.
const STATE_ADD: u32 = 1;

/// The main window, once known.
static MAIN: AtomicU32 = AtomicU32::new(0);

/// Note the main window (call every frame; it only does work once).
pub fn remember_main(frame: &eframe::Frame) {
    if MAIN.load(Ordering::Relaxed) != 0 {
        return;
    }
    if let Handle::X11(w) = window_of(frame) {
        MAIN.store(w, Ordering::Relaxed);
    }
}

/// What [`attach_dialog`] got to.
pub enum Attach {
    /// Not there yet (the window may not be mapped): try again next frame.
    Pending,
    /// Done; the window, if this is X11.
    Done(Option<u32>),
}

/// Find this process's window titled `title` and make it a dialog of the main window.
/// Not on X11: done at once, nothing to do.
pub fn attach_dialog(title: &str) -> Attach {
    let main = MAIN.load(Ordering::Relaxed);
    let Some((conn, screen)) = conn() else {
        return Attach::Done(None);
    };
    if main == 0 {
        return Attach::Pending;
    }
    let root = root(conn, *screen);
    match find(conn, root, title) {
        Ok(Some(w)) => {
            if let Err(e) = make_dialog(conn, root, w, main) {
                eprintln!("settings window not made a dialog: {e}");
            }
            Attach::Done(Some(w))
        }
        _ => Attach::Pending,
    }
}

/// Top-level windows: the window manager's client list, or the root's children without one.
fn find(conn: &RustConnection, root: Window, title: &str) -> XResult<Option<Window>> {
    let client_list = atom(conn, "_NET_CLIENT_LIST")?;
    let mut candidates: Vec<Window> = conn
        .get_property(false, root, client_list, AtomEnum::WINDOW, 0, MAX_WINDOWS)?
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
    Ok(candidates.into_iter().find(|&w| {
        let ours = conn
            .get_property(false, w, pid_atom, AtomEnum::CARDINAL, 0, 1)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().and_then(|mut v| v.next()))
            == Some(pid);
        let named = conn
            .get_property(false, w, name_atom, utf8, 0, MAX_TITLE)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|r| r.value == title.as_bytes());
        ours && named
    }))
}

/// Transient for `main`, typed as a dialog, and out of the taskbar and pager.
fn make_dialog(conn: &RustConnection, root: Window, w: Window, main: Window) -> XResult<()> {
    conn.change_property32(
        PropMode::REPLACE,
        w,
        AtomEnum::WM_TRANSIENT_FOR,
        AtomEnum::WINDOW,
        &[main],
    )?;
    let ty = atom(conn, "_NET_WM_WINDOW_TYPE")?;
    let dialog = atom(conn, "_NET_WM_WINDOW_TYPE_DIALOG")?;
    conn.change_property32(PropMode::REPLACE, w, ty, AtomEnum::ATOM, &[dialog])?;
    // _NET_WM_STATE on a mapped window is changed by asking the window manager.
    let state = atom(conn, "_NET_WM_STATE")?;
    let skip_taskbar = atom(conn, "_NET_WM_STATE_SKIP_TASKBAR")?;
    let skip_pager = atom(conn, "_NET_WM_STATE_SKIP_PAGER")?;
    let msg = ClientMessageEvent::new(32, w, state, [STATE_ADD, skip_taskbar, skip_pager, 1, 0]);
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        msg,
    )?;
    conn.flush()?;
    Ok(())
}
