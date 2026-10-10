//! Show new frames while the window is being drag-resized on X11.
//!
//! The treemap already rebuilds from the new size whenever a frame is drawn.
//! KWin will not put that frame on screen during the drag until the client
//! answers `_NET_WM_SYNC_REQUEST` by updating an XSync counter. Without that,
//! the old picture stays put and the newly exposed area stays empty until the
//! mouse button is released.

use eframe::egui::Context;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::thread;
use x11rb::connection::Connection;
use x11rb::protocol::record::{ConnectionExt as _, ExtRange, Range, Range16, Range8};
use x11rb::protocol::sync::{ConnectionExt as _, Int64};
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode, Window};
use x11rb::rust_connection::RustConnection;

struct State {
    conn: Mutex<RustConnection>,
    counter: u32,
    /// Counter values from `_NET_WM_SYNC_REQUEST`, oldest first.
    pending: Mutex<VecDeque<(u32, i32)>>,
}

static STATE: OnceLock<State> = OnceLock::new();

/// Opt the window into the resize protocol. Safe to call every frame.
pub fn install(frame: &eframe::Frame, ctx: &Context) {
    if STATE.get().is_some() {
        return;
    }
    static GAVE_UP: Mutex<bool> = Mutex::new(false);
    if *GAVE_UP.lock().expect("x11 resize flag") {
        return;
    }
    let window = match x11_window(frame) {
        WindowKind::Wait => return,
        WindowKind::Other => {
            *GAVE_UP.lock().expect("x11 resize flag") = true;
            return;
        }
        WindowKind::X11(window) => window,
    };
    if let Err(err) = start(window, ctx.clone()) {
        *GAVE_UP.lock().expect("x11 resize flag") = true;
        eprintln!("live resize unavailable: {err}");
    }
}

/// Answer resize requests now that this frame's treemap matches the window size.
pub fn acknowledge() {
    let Some(state) = STATE.get() else { return };
    let pending: Vec<_> = state
        .pending
        .lock()
        .expect("x11 resize queue")
        .drain(..)
        .collect();
    if pending.is_empty() {
        return;
    }
    let conn = state.conn.lock().expect("x11 resize connection");
    for (lo, hi) in pending {
        if conn
            .sync_set_counter(state.counter, Int64 { lo, hi })
            .is_err()
        {
            return;
        }
    }
    let _ = conn.flush();
}

fn start(window: Window, ctx: Context) -> Result<(), String> {
    let (conn, _) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let counter = conn.generate_id().map_err(|e| e.to_string())?;
    conn.sync_create_counter(counter, Int64 { hi: 0, lo: 0 })
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| e.to_string())?;

    let wm_protocols = atom(&conn, b"WM_PROTOCOLS")?;
    let sync_request = atom(&conn, b"_NET_WM_SYNC_REQUEST")?;
    let sync_counter = atom(&conn, b"_NET_WM_SYNC_REQUEST_COUNTER")?;
    let mut protocols = get_atoms(&conn, window, wm_protocols)?;
    if !protocols.contains(&sync_request) {
        protocols.push(sync_request);
    }
    put_atoms(&conn, window, wm_protocols, AtomEnum::ATOM, &protocols)?;
    put_atoms(&conn, window, sync_counter, AtomEnum::CARDINAL, &[counter])?;
    conn.flush().map_err(|e| e.to_string())?;

    // The event loop is single-threaded, so nothing else can set this first.
    let _ = STATE.set(State {
        conn: Mutex::new(conn),
        counter,
        pending: Mutex::new(VecDeque::new()),
    });

    thread::Builder::new()
        .name("x11-resize".into())
        .spawn(move || {
            if let Err(err) = watch(window, wm_protocols, sync_request, ctx) {
                eprintln!("live resize watch ended: {err}");
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Block on XRecord and queue each sync request for [`acknowledge`].
fn watch(window: Window, wm_protocols: u32, sync_request: u32, ctx: Context) -> Result<(), String> {
    let (conn, _) = x11rb::connect(None).map_err(|e| e.to_string())?;
    conn.record_query_version(1, 13)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    let context = conn.generate_id().map_err(|e| e.to_string())?;
    let range = Range {
        core_requests: none8(),
        core_replies: none8(),
        ext_requests: ExtRange {
            major: none8(),
            minor: none16(),
        },
        ext_replies: ExtRange {
            major: none8(),
            minor: none16(),
        },
        // ClientMessage is event 33. Leave every other range at 0..=0, which
        // XRecord treats as empty (a first greater than last is a BadValue).
        delivered_events: Range8 {
            first: 33,
            last: 33,
        },
        device_events: none8(),
        errors: none8(),
        client_started: false,
        client_died: false,
    };
    // 3 is XRecordAllClients: every client's ClientMessage events, filtered to our window below.
    conn.record_create_context(context, 0, &[3], &[range])
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| e.to_string())?;

    for reply in conn
        .record_enable_context(context)
        .map_err(|e| e.to_string())?
    {
        let reply = reply.map_err(|e| e.to_string())?;
        let mut found = false;
        for value in sync_values(
            &reply.data,
            reply.client_swapped,
            window,
            wm_protocols,
            sync_request,
        ) {
            if let Some(state) = STATE.get() {
                state
                    .pending
                    .lock()
                    .expect("x11 resize queue")
                    .push_back(value);
                found = true;
            }
        }
        if found {
            // ConfigureNotify for the new size follows this message. Wake a frame so
            // `acknowledge` runs once the treemap has been laid out for it.
            ctx.request_repaint();
        }
    }
    Ok(())
}

fn sync_values(
    data: &[u8],
    swapped: bool,
    window: u32,
    wm_protocols: u32,
    sync_request: u32,
) -> Vec<(u32, i32)> {
    let mut found = Vec::new();
    let mut i = 0;
    while i + 32 <= data.len() {
        let ev = &data[i..i + 32];
        i += 32;
        // Wire ClientMessage. SendEvent sets the high bit of the type byte.
        if ev[0] & 0x7f != 33 || ev[1] != 32 {
            continue;
        }
        if word(ev, 4, swapped) != window || word(ev, 8, swapped) != wm_protocols {
            continue;
        }
        if word(ev, 12, swapped) != sync_request {
            continue;
        }
        found.push((word(ev, 20, swapped), word(ev, 24, swapped) as i32));
    }
    found
}

/// Intercepted data is in the recorded client's byte order.
fn word(ev: &[u8], at: usize, swapped: bool) -> u32 {
    let raw = u32::from_ne_bytes([ev[at], ev[at + 1], ev[at + 2], ev[at + 3]]);
    if swapped {
        raw.swap_bytes()
    } else {
        raw
    }
}

enum WindowKind {
    Wait,
    Other,
    X11(Window),
}

fn x11_window(frame: &eframe::Frame) -> WindowKind {
    let Ok(handle) = frame.window_handle() else {
        return WindowKind::Wait;
    };
    match handle.as_raw() {
        RawWindowHandle::Xlib(window) => WindowKind::X11(window.window as u32),
        RawWindowHandle::Xcb(window) => WindowKind::X11(window.window.get()),
        _ => WindowKind::Other,
    }
}

fn atom(conn: &RustConnection, name: &[u8]) -> Result<u32, String> {
    Ok(conn
        .intern_atom(false, name)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?
        .atom)
}

fn get_atoms(conn: &RustConnection, window: Window, property: u32) -> Result<Vec<u32>, String> {
    let reply = conn
        .get_property(false, window, property, AtomEnum::ATOM, 0, 64)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    Ok(reply
        .value32()
        .map(|values| values.collect())
        .unwrap_or_default())
}

fn put_atoms(
    conn: &RustConnection,
    window: Window,
    property: u32,
    type_: impl Into<x11rb::protocol::xproto::Atom>,
    values: &[u32],
) -> Result<(), String> {
    let mut data = Vec::with_capacity(values.len() * 4);
    for value in values {
        data.extend_from_slice(&value.to_ne_bytes());
    }
    conn.change_property(
        PropMode::REPLACE,
        window,
        property,
        type_,
        32,
        values.len() as u32,
        &data,
    )
    .map_err(|e| e.to_string())?
    .check()
    .map_err(|e| e.to_string())
}

fn none8() -> Range8 {
    Range8 { first: 0, last: 0 }
}

fn none16() -> Range16 {
    Range16 { first: 0, last: 0 }
}
