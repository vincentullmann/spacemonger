//! The window shadow's pixels: eight ARGB tiles (four corners, four edges the window manager
//! repeats) for `_KDE_NET_WM_SHADOW`, and how far they reach outside the window.

use super::XResult;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat, Window};
use x11rb::rust_connection::RustConnection;

/// How far the shadow reaches past the window's sides, and how far down it's shifted.
const RADIUS: i32 = 22;
const OFFSET_Y: i32 = 4;
/// Opacity right at the window's edge.
const STRENGTH: f32 = 0.30;
/// Size of the stand-in window the tiles are worked out around.
const WIN: i32 = 1000;

/// A shadow tile's area (x, y, w, h) around the window.
type Tile = (i32, i32, i32, i32);

/// The `_KDE_NET_WM_SHADOW` property value: 8 pixmaps, then the top, right, bottom, left
/// paddings.
pub type ShadowProperty = [u32; 12];

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
    let win = WIN as f32;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let a = alpha(x as f32 + 0.5, y as f32 + 0.5, win, win);
            out.extend_from_slice(&[0, 0, 0, (a * 255.0).round() as u8]);
        }
    }
    out
}

/// The tiles in `_KDE_NET_WM_SHADOW` order (top, top-right, right, bottom-right, bottom,
/// bottom-left, left, top-left) as (x, y, w, h) around a 1000 × 1000 window, and the paddings.
fn layout() -> ([Tile; 8], [u32; 4]) {
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

/// Upload the tiles as 32-bit pixmaps (fails without a 32-bit depth) and build the property.
pub fn shadow_pixmaps(conn: &RustConnection, root: Window) -> XResult<ShadowProperty> {
    let (tiles, pads) = layout();
    let mut value: ShadowProperty = [0; 12];
    for (i, &(x, y, w, h)) in tiles.iter().enumerate() {
        let pixmap = conn.generate_id()?;
        conn.create_pixmap(32, pixmap, root, w as u16, h as u16)?
            .check()?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, pixmap, &Default::default())?;
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
        )?;
        conn.free_gc(gc)?;
        value[i] = pixmap;
    }
    value[8..].copy_from_slice(&pads);
    conn.flush()?;
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
        assert!(alpha(-0.5, 500.0, WIN as f32, WIN as f32) > 0.25);
        assert_eq!(alpha(-(RADIUS as f32), 500.0, WIN as f32, WIN as f32), 0.0);
    }
}
