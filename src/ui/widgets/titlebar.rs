//! The window's own title bar (system decorations are off): command icons on the left, the
//! title in the middle, minimise / maximise / close on the right. Empty space drags the
//! window; a double-click maximises it.

use super::CommandState;
use crate::core::actions::Action;
use crate::ui::keymap::{shortcut_text, Command, Keymap};
use eframe::egui::{
    self, Align, Button, Color32, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, Vec2,
    ViewportCommand,
};
use egui_phosphor::regular as icon;

/// Height of the bar.
pub const HEIGHT: f32 = 34.0;
const ICON_SIZE: f32 = 16.0;
const BUTTON: Vec2 = Vec2::new(30.0, 28.0);
const WINDOW_BUTTON_W: f32 = 44.0;
/// Pointer travel (points) with the button down before a press on the bar moves the window.
const DRAG_THRESHOLD: f32 = 3.0;
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xc4, 0x2b, 0x1c);

/// Draw the bar. Returns the command clicked, if any.
pub fn titlebar(ui: &mut Ui, st: &CommandState, keys: &Keymap, title: &str) -> Option<Action> {
    let full = ui.max_rect();
    // Behind everything, so it only gets the pointer where nothing else is.
    let bg = ui.interact(full, ui.id().with("titlebar_bg"), Sense::click_and_drag());
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));

    // Window buttons first, so the icons give way (clipped) in a narrow window, not them.
    let buttons_w = 3.0 * WINDOW_BUTTON_W;
    let right = Rect::from_min_max(egui::pos2(full.right() - buttons_w, full.top()), full.max);
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(right)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            window_buttons(ui, maximized);
        },
    );

    let left = Rect::from_min_max(full.min, egui::pos2(right.left(), full.bottom()));
    let mut cmds = ui.new_child(
        UiBuilder::new()
            .max_rect(left)
            .layout(Layout::left_to_right(Align::Center)),
    );
    cmds.set_clip_rect(left.intersect(ui.clip_rect()));
    cmds.spacing_mut().item_spacing.x = 2.0;
    cmds.add_space(6.0);
    let act = commands(&mut cmds, st, keys);

    let used = cmds.min_rect().right();
    let mid_left = (used + 12.0).min(left.right());
    let mid = Rect::from_min_max(
        egui::pos2(mid_left, full.top()),
        egui::pos2((left.right() - 12.0).max(mid_left), full.bottom()),
    );
    title_text(ui, mid, full, title);

    let icons = cmds.min_rect();
    move_or_maximize(
        ui,
        &bg,
        |p| !icons.contains(p) && !right.contains(p),
        maximized,
    );
    act
}

/// Drag empty bar space to move the window, double-click it to toggle maximised.
///
/// The move starts once the pointer has moved a few pixels with the button down, tracked
/// here from the raw pointer rather than egui's drag state, so it also works on the click
/// that focuses an inactive window. Then the window manager takes over (see `wm_grab`).
fn move_or_maximize(
    ui: &Ui,
    bg: &egui::Response,
    empty: impl Fn(egui::Pos2) -> bool,
    maximized: bool,
) {
    if bg.double_clicked() {
        ui.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        return;
    }
    let (pressed, down, pos) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.interact_pos(),
        )
    });
    let id = bg.id.with("press");
    if pressed {
        let start = pos.filter(|&p| bg.contains_pointer() && empty(p));
        ui.data_mut(|d| d.insert_temp(id, start));
        return;
    }
    let start: Option<egui::Pos2> = ui.data(|d| d.get_temp(id)).flatten();
    let Some(start) = start else { return };
    if !down {
        ui.data_mut(|d| d.remove::<Option<egui::Pos2>>(id));
    } else if pos.is_some_and(|p| p.distance(start) > DRAG_THRESHOLD) {
        ui.data_mut(|d| d.remove::<Option<egui::Pos2>>(id));
        super::wm_grab(ui.ctx(), ViewportCommand::StartDrag);
    }
}

/// The window title in `mid`, dimmed, centred on the whole bar (`full`) when it fits, cut
/// short with "…" when it doesn't.
fn title_text(ui: &Ui, mid: Rect, full: Rect, title: &str) {
    if mid.width() < 24.0 {
        return;
    }
    let color = ui.visuals().weak_text_color();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple_singleline(title.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(mid.width());
    let galley = ui.painter().layout_job(job);
    let size = galley.size();
    let half = size.x / 2.0;
    let cx = full.center().x.clamp(mid.left() + half, mid.right() - half);
    let pos = egui::pos2(cx - half, full.center().y - size.y / 2.0).round();
    ui.painter().galley(pos, galley, color);
}

/// "Open… (Ctrl+O)": the command's name and its first shortcut.
fn tip(keys: &Keymap, cmd: Command, name: &str) -> String {
    match keys.for_command(cmd).next() {
        Some((_, s)) => format!("{name}  ({})", shortcut_text(s)),
        None => name.to_owned(),
    }
}

fn icon_button(ui: &mut Ui, glyph: &str, enabled: bool, tip: &str) -> egui::Response {
    let b = Button::new(RichText::new(glyph).size(ICON_SIZE))
        .frame_when_inactive(false)
        .min_size(BUTTON);
    ui.add_enabled(enabled, b)
        .on_hover_text(tip)
        .on_disabled_hover_text(tip)
}

/// A short vertical line between icon groups.
fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(11.0, BUTTON.y), Sense::hover());
    let x = rect.center().x.round() + 0.5;
    let c = ui.visuals().widgets.noninteractive.bg_stroke.color;
    ui.painter().vline(
        x,
        rect.shrink2(Vec2::new(0.0, 7.0)).y_range(),
        Stroke::new(1.0, c),
    );
}

fn commands(ui: &mut Ui, st: &CommandState, keys: &Keymap) -> Option<Action> {
    let mut act = main_menu(ui, st, keys);
    let mut cmd = |ui: &mut Ui, glyph: &str, enabled: bool, c: Command, name: &str| {
        if icon_button(ui, glyph, enabled, &tip(keys, c, name)).clicked() {
            act = Some(c.action(false));
        }
    };
    cmd(ui, icon::FOLDER_OPEN, true, Command::Open, "Open…");
    cmd(
        ui,
        icon::ARROW_CLOCKWISE,
        st.has_tree,
        Command::Reload,
        "Reload",
    );
    divider(ui);
    icon_button(ui, icon::MAGNIFYING_GLASS, false, "Search (coming later)");
    act
}

/// The menu behind the first icon: every command the bar has, grouped.
fn main_menu(ui: &mut Ui, st: &CommandState, keys: &Keymap) -> Option<Action> {
    let button = Button::new(RichText::new(icon::LIST).size(ICON_SIZE))
        .frame_when_inactive(false)
        .min_size(BUTTON);
    let mut act = None;
    let (resp, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        ui.set_min_width(200.0);
        let mut item = |ui: &mut Ui, glyph: &str, label: &str, enabled: bool, c: Command| {
            let shortcut = keys
                .for_command(c)
                .next()
                .map(|(_, s)| shortcut_text(s))
                .unwrap_or_default();
            let b = Button::new((RichText::new(glyph), label)).shortcut_text(shortcut);
            if ui.add_enabled(enabled, b).clicked() {
                act = Some(c.action(false));
            }
        };
        item(ui, icon::FOLDER_OPEN, "Open…", true, Command::Open);
        item(
            ui,
            icon::ARROW_CLOCKWISE,
            "Reload",
            st.has_tree,
            Command::Reload,
        );
        ui.separator();
        let (zoom_in, zoomed) = (st.has_tree && st.sel_folder, st.has_tree && st.zoomed);
        item(
            ui,
            icon::MAGNIFYING_GLASS_PLUS,
            "Zoom in",
            zoom_in,
            Command::ZoomIn,
        );
        item(
            ui,
            icon::MAGNIFYING_GLASS_MINUS,
            "Zoom out",
            zoomed,
            Command::ZoomOut,
        );
        item(
            ui,
            icon::CORNERS_OUT,
            "Zoom to fit",
            zoomed,
            Command::ZoomFull,
        );
        ui.separator();
        let check = if st.show_free {
            icon::CHECK_SQUARE
        } else {
            icon::SQUARE
        };
        item(
            ui,
            check,
            "Show free space",
            st.has_tree,
            Command::ToggleFree,
        );
        let unhide = if st.hidden > 0 {
            format!("Unhide all ({})", st.hidden)
        } else {
            "Unhide all".to_owned()
        };
        item(ui, icon::EYE, &unhide, st.hidden > 0, Command::UnhideAll);
        ui.separator();
        item(ui, icon::GEAR_SIX, "Settings…", true, Command::Settings);
    });
    resp.on_hover_text("Menu");
    act
}

/// Close, maximise / restore, minimise (laid out right to left).
fn window_buttons(ui: &mut Ui, maximized: bool) {
    let h = ui.max_rect().height();
    let button = |ui: &mut Ui, glyph: &str, hover_fill: Option<Color32>, tip: &str| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(WINDOW_BUTTON_W, h), Sense::click());
        let visuals = ui.visuals();
        let mut color = visuals.text_color();
        if resp.hovered() {
            let fill = match hover_fill {
                Some(c) => {
                    color = Color32::WHITE;
                    c
                }
                None => visuals.widgets.hovered.weak_bg_fill,
            };
            let fill = if resp.is_pointer_button_down_on() {
                fill.gamma_multiply(0.8)
            } else {
                fill
            };
            ui.painter().rect_filled(rect, 0.0, fill);
        }
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(ICON_SIZE - 2.0),
            color,
        );
        resp.on_hover_text(tip).clicked()
    };
    if button(ui, icon::X, Some(CLOSE_HOVER), "Close") {
        ui.send_viewport_cmd(ViewportCommand::Close);
    }
    let (glyph, tip) = if maximized {
        (icon::COPY_SIMPLE, "Restore")
    } else {
        (icon::SQUARE, "Maximise")
    };
    if button(ui, glyph, None, tip) {
        ui.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    if button(ui, icon::MINUS, None, "Minimise") {
        ui.send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}
