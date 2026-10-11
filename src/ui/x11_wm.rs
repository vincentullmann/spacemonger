//! Window manager services for our undecorated window on X11, through the WM's own protocols:
//!
//! - Shadow: `_KDE_NET_WM_SHADOW` (what KWindowShadow uses). We hand KWin eight ARGB tiles
//!   (four corners, four edges it repeats) and how far they reach outside the window; KWin
//!   draws them. Window managers that don't know the property ignore it.
//! - Window menu: `_GTK_SHOW_WINDOW_MENU`, which GTK sends for its client-side title bars.
//!   The WM shows its own menu (keep above, move to desktop, …) at the given point. Only used
//!   when the WM lists it in `_NET_SUPPORTED`.
//!
//! Both go over a connection of our own, kept open for the life of the app: the shadow
//! pixmaps belong to it and would be freed with it.

use eframe::egui::Pos2;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, ImageFormat, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// How far the shadow reaches past the window's sides, and how far down it's shifted.
const RADIUS: i32 = 22;
const OFFSET_Y: i32 = 4;
/// Opacity right at the window's edge.
const STRENGTH: f32 = 0.30;

/// A shadow tile's area (x, y, w, h) around the window.
type Tile = (i32, i32, i32, i32);

struct Wm {
    conn: RustConnection,
    window: Window,
    root: Window,
    /// `_NET_WM_CM_S<screen>`: owned by the running compositor, if any.
    cm_atom: u32,
    /// Last compositor check: (when, whether one runs).
    composited: Mutex<Option<(Instant, bool)>>,
    shadow_atom: u32,
    /// Property value: 8 pixmaps, then the top, right, bottom, left paddings. `None` if the
    /// server has no 32-bit depth to draw them in.
    shadow: Option<[u32; 12]>,
    shadow_on: Mutex<Option<bool>>,
    /// `_GTK_SHOW_WINDOW_MENU`, if the WM supports it.
    menu_atom: Option<u32>,
}

static WM: OnceLock<Wm> = OnceLock::new();
static GAVE_UP: AtomicBool = AtomicBool::new(false);
/// Not an X11 window: Wayland, always composited.
static WAYLAND: AtomicBool = AtomicBool::new(false);

/// Connect once the window exists. Call every frame; it does its work once.
pub fn install(frame: &eframe::Frame) {
    if WM.get().is_some() || GAVE_UP.load(Ordering::Relaxed) {
        return;
    }
    let Ok(win) = frame.window_handle() else {
        return; // not created yet
    };
    let RawWindowHandle::Xlib(win) = win.as_raw() else {
        GAVE_UP.store(true, Ordering::Relaxed);
        WAYLAND.store(true, Ordering::Relaxed);
        return;
    };
    match setup(win.window as Window) {
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
        wm.conn
            .change_property32(
                PropMode::REPLACE,
                wm.window,
                wm.shadow_atom,
                AtomEnum::CARDINAL,
                &value,
            )
            .map(drop)
    } else {
        wm.conn.delete_property(wm.window, wm.shadow_atom).map(drop)
    };
    let _ = wm.conn.flush();
}

/// Give another of our windows (the settings window) the same shadow.
pub fn add_shadow(window: u32) {
    let Some(wm) = WM.get() else { return };
    let Some(value) = wm.shadow else { return };
    let _ = wm.conn.change_property32(
        PropMode::REPLACE,
        window,
        wm.shadow_atom,
        AtomEnum::CARDINAL,
        &value,
    );
    let _ = wm.conn.flush();
}

/// How long a compositor check holds before asking the server again.
const COMPOSITED_TTL: Duration = Duration::from_secs(1);

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
        .conn
        .get_selection_owner(wm.cm_atom)
        .ok()
        .and_then(|c| c.reply().ok())
        .is_some_and(|r| r.owner != x11rb::NONE);
    *last = Some((Instant::now(), on));
    on
}

/// Whether [`show_window_menu`] will do anything.
pub fn has_window_menu() -> bool {
    WM.get().is_some_and(|wm| wm.menu_atom.is_some())
}

/// Ask the WM for its window menu at `pos` (window pixels). Call on a button release: while a
/// button is held, the WM can't take the pointer for its menu.
pub fn show_window_menu(pos: Pos2) {
    let Some(wm) = WM.get() else { return };
    let Some(atom) = wm.menu_atom else { return };
    let Ok(Ok(at)) = wm
        .conn
        .translate_coordinates(wm.window, wm.root, pos.x as i16, pos.y as i16)
        .map(|c| c.reply())
    else {
        return;
    };
    // data: device id (2 = core pointer, as GTK sends), root x, root y.
    let data = [2, at.dst_x as i32 as u32, at.dst_y as i32 as u32, 0, 0];
    let event = ClientMessageEvent::new(32, wm.window, atom, data);
    let mask = EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT;
    let _ = wm.conn.send_event(false, wm.root, mask, event);
    let _ = wm.conn.flush();
}

fn setup(window: Window) -> Result<Wm, String> {
    let err = |e: &dyn std::fmt::Display| e.to_string();
    let (conn, screen) = x11rb::connect(None).map_err(|e| err(&e))?;
    let root = conn.setup().roots[screen].root;
    let atom = |name: &[u8]| -> Result<u32, String> {
        Ok(conn
            .intern_atom(false, name)
            .map_err(|e| err(&e))?
            .reply()
            .map_err(|e| err(&e))?
            .atom)
    };
    let shadow_atom = atom(b"_KDE_NET_WM_SHADOW")?;
    let cm_atom = atom(format!("_NET_WM_CM_S{screen}").as_bytes())?;
    let menu = atom(b"_GTK_SHOW_WINDOW_MENU")?;
    let supported = atom(b"_NET_SUPPORTED")?;
    let listed: Vec<u32> = conn
        .get_property(false, root, supported, AtomEnum::ATOM, 0, 4096)
        .map_err(|e| err(&e))?
        .reply()
        .map_err(|e| err(&e))?
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default();
    let shadow = shadow_pixmaps(&conn, root).ok();
    Ok(Wm {
        conn,
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

/// Shadow opacity at `(x, y)` in window coordinates, for a window spanning `0..w` × `0..h`:
/// fades out with the distance from the window's outline moved down by [`OFFSET_Y`].
fn alpha(x: f32, y: f32, w: f32, h: f32) -> f32 {
    let (top, bottom) = (OFFSET_Y as f32, h + OFFSET_Y as f32);
    let dx = (-x).max(x - w).max(0.0);
    let dy = (top - y).max(y - bottom).max(0.0);
    let d = dx.hypot(dy) / RADIUS as f32;
    if d >= 1.0 {
        return 0.0;
    }
    // Gaussian-like falloff that reaches zero at the radius.
    STRENGTH * (1.0 - d).powi(2) * (-3.0 * d * d).exp()
}

/// One tile covering `x0..x0+w` × `y0..y0+h` of the area around a large window, as
/// premultiplied BGRA (black, so only alpha is non-zero).
fn tile(x0: i32, y0: i32, w: i32, h: i32) -> Vec<u8> {
    const WIN: f32 = 1000.0;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let a = alpha(x as f32 + 0.5, y as f32 + 0.5, WIN, WIN);
            out.extend_from_slice(&[0, 0, 0, (a * 255.0).round() as u8]);
        }
    }
    out
}

/// The tiles in `_KDE_NET_WM_SHADOW` order (top, top-right, right, bottom-right, bottom,
/// bottom-left, left, top-left) as (x, y, w, h) around a 1000 × 1000 window, and the paddings.
fn layout() -> ([Tile; 8], [u32; 4]) {
    const WIN: i32 = 1000;
    let (r, pt, pb) = (RADIUS, RADIUS - OFFSET_Y, RADIUS + OFFSET_Y);
    let mid = WIN / 2;
    let tiles = [
        (mid, -pt, 1, pt), // top
        (WIN, -pt, r, pt), // top-right
        (WIN, mid, r, 1),  // right
        (WIN, WIN, r, pb), // bottom-right
        (mid, WIN, 1, pb), // bottom
        (-r, WIN, r, pb),  // bottom-left
        (-r, mid, r, 1),   // left
        (-r, -pt, r, pt),  // top-left
    ];
    (tiles, [pt as u32, r as u32, pb as u32, r as u32])
}

fn shadow_pixmaps(conn: &RustConnection, root: Window) -> Result<[u32; 12], String> {
    let err = |e: &dyn std::fmt::Display| e.to_string();
    let (tiles, pads) = layout();
    let mut value = [0u32; 12];
    for (i, &(x, y, w, h)) in tiles.iter().enumerate() {
        let pixmap = conn.generate_id().map_err(|e| err(&e))?;
        conn.create_pixmap(32, pixmap, root, w as u16, h as u16)
            .map_err(|e| err(&e))?
            .check()
            .map_err(|e| err(&e))?;
        let gc = conn.generate_id().map_err(|e| err(&e))?;
        conn.create_gc(gc, pixmap, &Default::default())
            .map_err(|e| err(&e))?;
        conn.put_image(
            ImageFormat::Z_PIXMAP,
            pixmap,
            gc,
            w as u16,
            h as u16,
            0,
            0,
            0,
            32,
            &tile(x, y, w, h),
        )
        .map_err(|e| err(&e))?;
        conn.free_gc(gc).map_err(|e| err(&e))?;
        value[i] = pixmap;
    }
    value[8..].copy_from_slice(&pads);
    conn.flush().map_err(|e| err(&e))?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_tiles_fade_out() {
        let (tiles, pads) = layout();
        assert_eq!(pads, [18, 22, 26, 22]);
        for &(x, y, w, h) in &tiles {
            let px = tile(x, y, w, h);
            assert_eq!(px.len(), (w * h * 4) as usize);
            // Only alpha is set, and the far end of every tile is fully clear.
            assert!(px.chunks(4).all(|p| p[..3] == [0, 0, 0]));
        }
        // Strongest right at the edge, gone at the radius.
        assert!(alpha(-0.5, 500.0, 1000.0, 1000.0) > 0.25);
        assert_eq!(alpha(-(RADIUS as f32), 500.0, 1000.0, 1000.0), 0.0);
    }
}
