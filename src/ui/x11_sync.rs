//! Smooth interactive resizing on X11, the way Qt and GTK do it.
//!
//! winit doesn't take part in the window manager's resize handshake, so during a drag KWin
//! shows each new size straight away, and the uncovered area stays black until our frame for
//! that size lands. This module adds the missing pieces to winit's window:
//!
//! - `_NET_WM_SYNC_REQUEST`: before each resize step the WM sends a counter value and waits
//!   until we set our XSync counter to it, which we do once the frame for that size is shown.
//!   The WM therefore never shows a size we haven't drawn.
//! - Background colour and `NorthWestGravity`: if a frame is late anyway, the old picture
//!   stays pinned top-left and the new strip is filled with the theme background, not black.
//!
//! The request arrives as a ClientMessage on winit's own Xlib connection, and winit drops it.
//! We read it with Xlib's per-display event hook (`XESetWireToEvent`), which runs inside
//! winit's event pump on the main thread. Everything else uses a second connection.
//!
//! Per frame: [`frame_drawn`] at the end of `ui` marks the requests received so far as drawn
//! and asks for one more frame; [`acknowledge`] at the start of that frame (in `logic`, after
//! the drawn frame was presented) sets the counter.

use eframe::egui::{Color32, Context};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use x11_dl::xlib;
use x11rb::connection::Connection;
use x11rb::protocol::sync::{ConnectionExt as _, Int64};
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, Gravity, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

type WireToEvent =
    unsafe extern "C" fn(*mut xlib::Display, *mut xlib::XEvent, *mut xlib::xEvent) -> c_int;

/// Our window and the atoms the hook matches on. Written once in [`install`], before the hook.
static WINDOW: AtomicU32 = AtomicU32::new(0);
static WM_PROTOCOLS: AtomicU32 = AtomicU32::new(0);
static SYNC_REQUEST: AtomicU32 = AtomicU32::new(0);
/// Xlib's own ClientMessage converter, which the hook wraps.
static PREV_HOOK: OnceLock<Option<WireToEvent>> = OnceLock::new();
/// To wake a frame when a request arrives without a size change.
static CTX: OnceLock<Context> = OnceLock::new();

/// Latest request not yet drawn, and latest request drawn but not yet acknowledged.
static RECEIVED: Mutex<Option<i64>> = Mutex::new(None);
static DRAWN: Mutex<Option<i64>> = Mutex::new(None);

struct X11 {
    conn: RustConnection,
    window: Window,
    counter: u32,
    depth: u8,
    background: Mutex<Option<Color32>>,
}

static X11: OnceLock<X11> = OnceLock::new();
static GAVE_UP: AtomicBool = AtomicBool::new(false);

/// Set up the handshake for the main window. Call every frame; it does its work once,
/// as soon as the window exists (eframe keeps it unmapped until the first frame is drawn).
pub fn install(frame: &eframe::Frame, ctx: &Context, background: Color32) {
    if X11.get().is_some() || GAVE_UP.load(Ordering::Relaxed) {
        return;
    }
    let (Ok(win), Ok(disp)) = (frame.window_handle(), frame.display_handle()) else {
        return; // not created yet
    };
    let (RawWindowHandle::Xlib(win), RawDisplayHandle::Xlib(disp)) = (win.as_raw(), disp.as_raw())
    else {
        GAVE_UP.store(true, Ordering::Relaxed); // Wayland, or not Xlib-backed
        return;
    };
    let Some(display) = disp.display else {
        GAVE_UP.store(true, Ordering::Relaxed);
        return;
    };
    match setup(win.window as Window, display.as_ptr().cast(), background) {
        Ok(x) => {
            let _ = CTX.set(ctx.clone());
            let _ = X11.set(x);
        }
        Err(err) => {
            GAVE_UP.store(true, Ordering::Relaxed);
            eprintln!("smooth resize unavailable: {err}");
        }
    }
}

/// Update the colour shown in freshly uncovered areas (theme change).
pub fn set_background(color: Color32) {
    let Some(x) = X11.get() else { return };
    let mut bg = x.background.lock().expect("x11 background");
    if *bg == Some(color) {
        return;
    }
    *bg = Some(color);
    let aux = ChangeWindowAttributesAux::new().background_pixel(pixel(color, x.depth));
    let _ = x.conn.change_window_attributes(x.window, &aux);
    let _ = x.conn.flush();
}

/// End of `ui`: this frame shows every request received so far. Ask for one more frame,
/// which acknowledges them once this one is on screen.
pub fn frame_drawn(ctx: &Context) {
    let Some(v) = RECEIVED.lock().expect("x11 sync").take() else {
        return;
    };
    *DRAWN.lock().expect("x11 sync") = Some(v);
    ctx.request_repaint();
}

/// Start of a frame (`logic`): the previous frame is presented, so release the WM.
pub fn acknowledge() {
    let Some(v) = DRAWN.lock().expect("x11 sync").take() else {
        return;
    };
    let Some(x) = X11.get() else { return };
    let value = Int64 {
        hi: (v >> 32) as i32,
        lo: v as u32,
    };
    let _ = x.conn.sync_set_counter(x.counter, value);
    let _ = x.conn.flush();
}

fn setup(window: Window, display: *mut xlib::Display, background: Color32) -> Result<X11, String> {
    let err = |e: &dyn std::fmt::Display| e.to_string();
    let (conn, _) = x11rb::connect(None).map_err(|e| err(&e))?;
    conn.sync_initialize(3, 1)
        .map_err(|e| err(&e))?
        .reply()
        .map_err(|e| err(&e))?;

    let atom = |name: &[u8]| -> Result<u32, String> {
        Ok(conn
            .intern_atom(false, name)
            .map_err(|e| err(&e))?
            .reply()
            .map_err(|e| err(&e))?
            .atom)
    };
    let wm_protocols = atom(b"WM_PROTOCOLS")?;
    let sync_request = atom(b"_NET_WM_SYNC_REQUEST")?;
    let sync_counter = atom(b"_NET_WM_SYNC_REQUEST_COUNTER")?;

    let counter = conn.generate_id().map_err(|e| err(&e))?;
    conn.sync_create_counter(counter, Int64 { hi: 0, lo: 0 })
        .map_err(|e| err(&e))?
        .check()
        .map_err(|e| err(&e))?;

    // Advertise the protocol next to winit's WM_DELETE_WINDOW / _NET_WM_PING.
    let mut protocols: Vec<u32> = conn
        .get_property(false, window, wm_protocols, AtomEnum::ATOM, 0, 64)
        .map_err(|e| err(&e))?
        .reply()
        .map_err(|e| err(&e))?
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default();
    if !protocols.contains(&sync_request) {
        protocols.push(sync_request);
    }
    conn.change_property32(
        PropMode::REPLACE,
        window,
        wm_protocols,
        AtomEnum::ATOM,
        &protocols,
    )
    .map_err(|e| err(&e))?;
    conn.change_property32(
        PropMode::REPLACE,
        window,
        sync_counter,
        AtomEnum::CARDINAL,
        &[counter],
    )
    .map_err(|e| err(&e))?;

    let depth = conn
        .get_geometry(window)
        .map_err(|e| err(&e))?
        .reply()
        .map_err(|e| err(&e))?
        .depth;
    let aux = ChangeWindowAttributesAux::new()
        .background_pixel(pixel(background, depth))
        .bit_gravity(Gravity::NORTH_WEST);
    conn.change_window_attributes(window, &aux)
        .map_err(|e| err(&e))?
        .check()
        .map_err(|e| err(&e))?;

    // Hook winit's connection. Store the atoms first: the hook may run on the next event.
    WINDOW.store(window, Ordering::Relaxed);
    WM_PROTOCOLS.store(wm_protocols, Ordering::Relaxed);
    SYNC_REQUEST.store(sync_request, Ordering::Relaxed);
    let xlib = xlib::Xlib::open().map_err(|e| err(&e))?;
    // SAFETY: `display` is winit's live Xlib display; we're on the thread that pumps it, and
    // not inside an Xlib call. The hook only ever forwards to Xlib's own converter.
    let prev = unsafe { (xlib.XESetWireToEvent)(display, xlib::ClientMessage, Some(hook)) };
    let _ = PREV_HOOK.set(prev);

    Ok(X11 {
        conn,
        window,
        counter,
        depth,
        background: Mutex::new(Some(background)),
    })
}

/// Wraps Xlib's ClientMessage converter and notes `_NET_WM_SYNC_REQUEST` values for our window.
unsafe extern "C" fn hook(
    dpy: *mut xlib::Display,
    ev: *mut xlib::XEvent,
    wire: *mut xlib::xEvent,
) -> c_int {
    let Some(Some(prev)) = PREV_HOOK.get().copied() else {
        return 0;
    };
    // SAFETY: same arguments Xlib passed us.
    let keep = unsafe { prev(dpy, ev, wire) };
    if keep != 0 {
        // SAFETY: `prev` filled `ev` as a ClientMessage.
        let cm = unsafe { &(*ev).client_message };
        if cm.window as u32 == WINDOW.load(Ordering::Relaxed)
            && cm.message_type as u32 == WM_PROTOCOLS.load(Ordering::Relaxed)
            && cm.format == 32
            && cm.data.get_long(0) as u32 == SYNC_REQUEST.load(Ordering::Relaxed)
        {
            let lo = cm.data.get_long(2) as u32 as i64;
            let hi = cm.data.get_long(3) as i32 as i64;
            *RECEIVED.lock().expect("x11 sync") = Some((hi << 32) | lo);
            // Usually a ConfigureNotify follows and wakes a frame anyway; this covers the rest.
            if let Some(ctx) = CTX.get() {
                ctx.request_repaint();
            }
        }
    }
    keep
}

/// Pixel value for a TrueColor visual of `depth` bits (32 carries alpha).
fn pixel(c: Color32, depth: u8) -> u32 {
    let rgb = (c.r() as u32) << 16 | (c.g() as u32) << 8 | c.b() as u32;
    if depth == 32 {
        0xFF00_0000 | rgb
    } else {
        rgb
    }
}
