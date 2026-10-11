//! Window manager services for our undecorated window on X11, through the WM's own protocols:
//!
//! - Shadow: `_KDE_NET_WM_SHADOW` (what KWindowShadow uses). We hand KWin eight ARGB tiles
//!   (four corners, four edges it repeats) and how far they reach outside the window; KWin
//!   draws them. Window managers that don't know the property ignore it.
//! - Window menu: `_GTK_SHOW_WINDOW_MENU`, which GTK sends for its client-side title bars.
//!   The WM shows its own menu (keep above, move to desktop, …) at the given point. Only used
//!   when the WM lists it in `_NET_SUPPORTED`.
//!
//! Both go over the app's own connection (see [`super::conn`]).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use eframe::egui::Pos2;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::shadow::{shadow_pixmaps, ShadowProperty};
use super::{atom, conn, root, window_of, Handle, XResult};

/// How long a compositor check holds before asking the server again.
const COMPOSITED_TTL: Duration = Duration::from_secs(1);
/// How many atoms of `_NET_SUPPORTED` are read.
const MAX_SUPPORTED: u32 = 4096;
/// Device id in `_GTK_SHOW_WINDOW_MENU` (the core pointer, as GTK sends).
const CORE_POINTER: u32 = 2;

/// What the window manager services need, set up once the main window exists.
struct Wm {
    window: Window,
    root: Window,
    /// `_NET_WM_CM_S<screen>`: owned by the running compositor, if any.
    cm_atom: u32,
    /// Last compositor check: (when, whether one runs).
    composited: Mutex<Option<(Instant, bool)>>,
    shadow_atom: u32,
    /// `None` if the server has no 32-bit depth to draw the tiles in.
    shadow: Option<ShadowProperty>,
    shadow_on: Mutex<Option<bool>>,
    /// `_GTK_SHOW_WINDOW_MENU`, if the WM supports it.
    menu_atom: Option<u32>,
}

static WM: OnceLock<Wm> = OnceLock::new();
static GAVE_UP: AtomicBool = AtomicBool::new(false);
/// Not an X11 window: Wayland, always composited.
static WAYLAND: AtomicBool = AtomicBool::new(false);

/// Set up once the window exists. Call every frame; it does its work once.
pub fn install_wm(frame: &eframe::Frame) {
    if WM.get().is_some() || GAVE_UP.load(Ordering::Relaxed) {
        return;
    }
    let window = match window_of(frame) {
        Handle::Pending => return,
        Handle::X11(w) => w,
        Handle::Other => {
            GAVE_UP.store(true, Ordering::Relaxed);
            WAYLAND.store(true, Ordering::Relaxed);
            return;
        }
    };
    match setup(window) {
        Ok(wm) => {
            let _ = WM.set(wm);
        }
        Err(e) => {
            GAVE_UP.store(true, Ordering::Relaxed);
            eprintln!("window shadow / menu unavailable: {e}");
        }
    }
}

/// Show the shadow, or not (maximised and full-screen windows have none).
pub fn set_shadow(on: bool) {
    let Some(wm) = WM.get() else { return };
    let Some(value) = wm.shadow else { return };
    let mut cur = wm.shadow_on.lock().expect("x11 shadow");
    if *cur == Some(on) {
        return;
    }
    *cur = Some(on);
    let _ = if on {
        wm.conn()
            .change_property32(
                PropMode::REPLACE,
                wm.window,
                wm.shadow_atom,
                AtomEnum::CARDINAL,
                &value,
            )
            .map(drop)
    } else {
        wm.conn()
            .delete_property(wm.window, wm.shadow_atom)
            .map(drop)
    };
    let _ = wm.conn().flush();
}

/// Give another of our windows (the settings window) the same shadow.
pub fn add_shadow(window: u32) {
    let Some(wm) = WM.get() else { return };
    let Some(value) = wm.shadow else { return };
    let _ = wm.conn().change_property32(
        PropMode::REPLACE,
        window,
        wm.shadow_atom,
        AtomEnum::CARDINAL,
        &value,
    );
    let _ = wm.conn().flush();
}

/// Whether a compositor is running, so windows can have see-through (rounded) corners.
/// Asks the X server at most once a second. Not on X11 (Wayland): always.
pub fn composited() -> bool {
    let Some(wm) = WM.get() else {
        return WAYLAND.load(Ordering::Relaxed);
    };
    let mut last = wm.composited.lock().expect("x11 composited");
    if let Some((at, on)) = *last {
        if at.elapsed() < COMPOSITED_TTL {
            return on;
        }
    }
    let on = wm
        .conn()
        .get_selection_owner(wm.cm_atom)
        .ok()
        .and_then(|c| c.reply().ok())
        .is_some_and(|r| r.owner != x11rb::NONE);
    *last = Some((Instant::now(), on));
    on
}

/// Ask the WM for its window menu at `pos` (window pixels). Call on a button release: while a
/// button is held, the WM can't take the pointer for its menu.
pub fn show_window_menu(pos: Pos2) {
    let Some(wm) = WM.get() else { return };
    let Some(atom) = wm.menu_atom else { return };
    let Ok(Ok(at)) = wm
        .conn()
        .translate_coordinates(wm.window, wm.root, pos.x as i16, pos.y as i16)
        .map(|c| c.reply())
    else {
        return;
    };
    // data: device id (2 = core pointer, as GTK sends), root x, root y.
    let data = [
        CORE_POINTER,
        at.dst_x as i32 as u32,
        at.dst_y as i32 as u32,
        0,
        0,
    ];
    let event = ClientMessageEvent::new(32, wm.window, atom, data);
    let mask = EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT;
    let _ = wm.conn().send_event(false, wm.root, mask, event);
    let _ = wm.conn().flush();
}

fn setup(window: Window) -> XResult<Wm> {
    let (conn, screen) = conn().ok_or("no X connection")?;
    let root = root(conn, *screen);
    let shadow_atom = atom(conn, "_KDE_NET_WM_SHADOW")?;
    let cm_atom = atom(conn, &format!("_NET_WM_CM_S{screen}"))?;
    let menu = atom(conn, "_GTK_SHOW_WINDOW_MENU")?;
    let supported = atom(conn, "_NET_SUPPORTED")?;
    let listed: Vec<u32> = conn
        .get_property(false, root, supported, AtomEnum::ATOM, 0, MAX_SUPPORTED)?
        .reply()?
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default();
    let shadow = shadow_pixmaps(conn, root).ok();
    Ok(Wm {
        window,
        root,
        cm_atom,
        composited: Mutex::new(None),
        shadow_atom,
        shadow,
        shadow_on: Mutex::new(None),
        menu_atom: listed.contains(&menu).then_some(menu),
    })
}

impl Wm {
    /// The shared connection (always there once `setup` succeeded).
    fn conn(&self) -> &'static RustConnection {
        &conn().expect("set up with a connection").0
    }
}
