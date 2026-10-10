//! What the system frame did before decorations were turned off: a thin border and resizing
//! from the window's edges and corners. Neither applies while maximised or full screen.

use eframe::egui::{
    self, CursorIcon, Id, LayerId, Order, Rect, ResizeDirection, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, ViewportCommand,
};

/// Width of the grab zone along each edge.
const EDGE: f32 = 5.0;
/// Length of the corner zones along each edge.
const CORNER: f32 = 14.0;

/// Call last in the frame, so the edge cursors win over whatever is underneath.
pub fn window_frame(ctx: &egui::Context) {
    let (maximized, fullscreen) = ctx.input(|i| {
        let v = i.viewport();
        (v.maximized.unwrap_or(false), v.fullscreen.unwrap_or(false))
    });
    if maximized || fullscreen {
        return;
    }
    let r = ctx.content_rect();
    let layer = LayerId::new(Order::Foreground, Id::new("window_frame"));

    let color = ctx
        .global_style()
        .visuals
        .widgets
        .noninteractive
        .bg_stroke
        .color;
    ctx.layer_painter(layer)
        .rect_stroke(r, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);

    use ResizeDirection as D;
    let (x0, x1, y0, y1) = (r.left(), r.right(), r.top(), r.bottom());
    let zone =
        |ax: f32, ay: f32, bx: f32, by: f32| Rect::from_min_max((ax, ay).into(), (bx, by).into());
    // Corners are L-shaped (two strips each); the edges stop where the corners start, so no
    // two directions overlap.
    let (c, e) = (CORNER, EDGE);
    let zones = [
        (zone(x0, y0, x0 + c, y0 + e), D::NorthWest),
        (zone(x0, y0, x0 + e, y0 + c), D::NorthWest),
        (zone(x1 - c, y0, x1, y0 + e), D::NorthEast),
        (zone(x1 - e, y0, x1, y0 + c), D::NorthEast),
        (zone(x0, y1 - e, x0 + c, y1), D::SouthWest),
        (zone(x0, y1 - c, x0 + e, y1), D::SouthWest),
        (zone(x1 - c, y1 - e, x1, y1), D::SouthEast),
        (zone(x1 - e, y1 - c, x1, y1), D::SouthEast),
        (zone(x0 + c, y0, x1 - c, y0 + e), D::North),
        (zone(x0 + c, y1 - e, x1 - c, y1), D::South),
        (zone(x0, y0 + c, x0 + e, y1 - c), D::West),
        (zone(x1 - e, y0 + c, x1, y1 - c), D::East),
    ];
    let ui = Ui::new(
        ctx.clone(),
        Id::new("window_frame_ui"),
        UiBuilder::new().layer_id(layer).max_rect(r),
    );
    let pressed = ctx.input(|i| i.pointer.primary_pressed());
    for (i, (rect, dir)) in zones.into_iter().enumerate() {
        let resp = ui.interact(rect, Id::new(("window_edge", i)), Sense::drag());
        if resp.hovered() || resp.is_pointer_button_down_on() {
            ctx.set_cursor_icon(cursor(dir));
            if pressed {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
            }
            break;
        }
    }
}

fn cursor(dir: ResizeDirection) -> CursorIcon {
    use ResizeDirection as D;
    match dir {
        D::North => CursorIcon::ResizeNorth,
        D::South => CursorIcon::ResizeSouth,
        D::East => CursorIcon::ResizeEast,
        D::West => CursorIcon::ResizeWest,
        D::NorthEast => CursorIcon::ResizeNorthEast,
        D::NorthWest => CursorIcon::ResizeNorthWest,
        D::SouthEast => CursorIcon::ResizeSouthEast,
        D::SouthWest => CursorIcon::ResizeSouthWest,
    }
}
