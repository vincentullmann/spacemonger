//! The look every dialog shares: the main menu's rounded frame (corner radius, outline,
//! shadow), a header with the title and an optional × button, and the body below it.
//!
//! [`Dialog`] shows one over the main window, movable by dragging any empty part of it. A
//! modal one dims the window behind it and takes all input. The settings window draws the
//! same [`header`] in a window of its own.

use eframe::egui::{
    self, Align, Align2, Area, Button, Color32, Frame, Id, Layout, Margin, Order, Rect, RichText,
    Sense, Ui, UiBuilder, UiKind, Vec2,
};
use egui_phosphor::regular as icon;

/// Height of the header row.
pub const HEADER_H: f32 = 40.0;
/// Space between the dialog's edge and its contents.
pub const PADDING: f32 = 14.0;
const CLOSE_BUTTON: Vec2 = Vec2::new(28.0, 28.0);
const ICON_SIZE: f32 = 16.0;
const BACKDROP: Color32 = Color32::from_black_alpha(90);

/// Where a dialog sits and how far it's been moved, kept between frames.
#[derive(Clone, Copy, Default)]
struct Placement {
    offset: Vec2,
    size: Option<Vec2>,
}

pub struct Dialog<'a> {
    id: Id,
    title: &'a str,
    modal: bool,
    closable: bool,
    pivot: Align2,
    margin: Vec2,
    width: f32,
}

pub struct DialogResponse<R> {
    pub inner: R,
    /// × clicked, or Escape on a modal dialog.
    pub close: bool,
}

impl<'a> Dialog<'a> {
    /// A modal dialog with a × button, centred to begin with.
    pub fn new(id_salt: &str, title: &'a str) -> Self {
        Self {
            id: Id::new(("dialog", id_salt)),
            title,
            modal: true,
            closable: true,
            pivot: Align2::CENTER_CENTER,
            margin: Vec2::ZERO,
            width: 420.0,
        }
    }

    /// Leave the rest of the window usable (and don't dim it).
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Start out in this corner (or edge, or centre) of the window, `margin` in from it.
    pub fn anchor(mut self, pivot: Align2, margin: Vec2) -> Self {
        self.pivot = pivot;
        self.margin = margin;
        self
    }

    /// Width of the contents.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn show<R>(self, ctx: &egui::Context, add: impl FnOnce(&mut Ui) -> R) -> DialogResponse<R> {
        let screen = ctx.content_rect();
        let mut place: Placement = ctx.data(|d| d.get_temp(self.id)).unwrap_or_default();
        let size = place
            .size
            .unwrap_or(Vec2::new(self.width + 2.0 * PADDING, 120.0));
        let start = self
            .pivot
            .align_size_within_rect(size, screen.shrink2(self.margin))
            .min;
        let min = clamp_into(start + place.offset, size, screen);

        let area = Area::new(self.id)
            .kind(if self.modal {
                UiKind::Modal
            } else {
                UiKind::Window
            })
            .order(if self.modal {
                Order::Foreground
            } else {
                Order::Middle
            })
            .fixed_pos(if self.modal { screen.min } else { min })
            .constrain(false)
            .interactable(true);
        let top_modal = self.modal
            && ctx.memory_mut(|m| {
                m.set_modal_layer(area.layer());
                m.top_modal_layer() == Some(area.layer())
            });

        let (title, closable, width, modal) = (self.title, self.closable, self.width, self.modal);
        let shown = area.show(ctx, |ui| {
            if modal {
                // Swallows clicks and drags outside the dialog.
                ui.painter().rect_filled(screen, 0.0, BACKDROP);
                ui.interact(screen, ui.id().with("backdrop"), Sense::click_and_drag());
            }
            let rect = Rect::from_min_size(min, Vec2::new(size.x, screen.height().max(size.y)));
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(rect)
                    .sense(Sense::click_and_drag()),
                |ui| framed(ui, title, closable, width, add),
            )
        });
        let body = shown.inner.response;
        let (mut close, inner) = shown.inner.inner;

        if body.dragged() {
            place.offset += body.drag_delta();
            // Don't let it wander further off than it can be shown.
            place.offset = clamp_into(start + place.offset, size, screen) - start;
        }
        let new_size = body.rect.size();
        if place.size != Some(new_size) {
            if place.size.is_none() {
                ctx.request_discard("dialog sizing");
            }
            place.size = Some(new_size);
        }
        ctx.data_mut(|d| d.insert_temp(self.id, place));

        if top_modal && !egui::Popup::is_any_open(ctx) {
            close |= ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        }
        DialogResponse { inner, close }
    }
}

/// Move `min` so a `size` box from there stays inside `screen` (top-left wins if too big).
fn clamp_into(min: egui::Pos2, size: Vec2, screen: Rect) -> egui::Pos2 {
    egui::pos2(
        min.x.min(screen.right() - size.x).max(screen.left()),
        min.y.min(screen.bottom() - size.y).max(screen.top()),
    )
}

/// The frame, header and padded body. Returns whether × was clicked, and what `add` returned.
fn framed<R>(
    ui: &mut Ui,
    title: &str,
    closable: bool,
    width: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> (bool, R) {
    Frame::menu(ui.style())
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.set_width(width + 2.0 * PADDING);
            let spacing = ui.spacing().item_spacing;
            ui.spacing_mut().item_spacing.y = 0.0;
            let close = header(ui, title, closable);
            let inner = Frame::NONE
                .inner_margin(Margin {
                    left: PADDING as i8,
                    right: PADDING as i8,
                    top: 2,
                    bottom: PADDING as i8,
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = spacing;
                    ui.set_width(width);
                    add(ui)
                })
                .inner;
            (close, inner)
        })
        .inner
}

/// The title on the left, × on the right (if `closable`), across the available width.
/// Returns whether × was clicked.
pub fn header(ui: &mut Ui, title: &str, closable: bool) -> bool {
    let size = Vec2::new(ui.available_width(), HEADER_H);
    ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_size(size);
        ui.add_space(PADDING);
        // Not selectable, so dragging the title moves the dialog.
        ui.add(egui::Label::new(RichText::new(title).strong()).selectable(false));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(PADDING - (CLOSE_BUTTON.x - ICON_SIZE) / 2.0);
            closable && close_button(ui).clicked()
        })
        .inner
    })
    .inner
}

/// A frameless × the size of the title bar's icon buttons.
pub fn close_button(ui: &mut Ui) -> egui::Response {
    let b = Button::new(RichText::new(icon::X).size(ICON_SIZE))
        .frame_when_inactive(false)
        .min_size(CLOSE_BUTTON);
    ui.add(b).on_hover_text("Close")
}
