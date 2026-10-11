//! The window's own title bar (system decorations are off): command icons on the left, the
//! title (or scan progress) in the middle, minimise / maximise / close on the right. Empty
//! space drags the window, a double-click maximises it, a right-click opens the window
//! manager's window menu. The bar dims while the window isn't focused.

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
/// The bar's opacity while the window isn't focused.
const UNFOCUSED_OPACITY: f32 = 0.55;
const SEARCH_W: f32 = 260.0;
const PROGRESS_H: f32 = 2.0;
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xc4, 0x2b, 0x1c);

/// Draw the bar. `progress` (0..=1) draws a progress line along its bottom edge. Returns the
/// command clicked, if any.
pub fn titlebar(
    ui: &mut Ui,
    st: &CommandState,
    keys: &Keymap,
    title: &str,
    progress: Option<f32>,
) -> Option<Action> {
    let full = ui.max_rect();
    if !ui.input(|i| i.viewport().focused.unwrap_or(true)) {
        ui.multiply_opacity(UNFOCUSED_OPACITY);
    }
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
    let (act, search_hovered) = commands(&mut cmds, st, keys);

    let used = cmds.min_rect().right();
    let mid_left = (used + 12.0).min(left.right());
    let mid = Rect::from_min_max(
        egui::pos2(mid_left, full.top()),
        egui::pos2((left.right() - 12.0).max(mid_left), full.bottom()),
    );
    if search_hovered {
        search_placeholder(ui, mid, full);
    } else {
        title_text(ui, mid, full, title);
    }
    if let Some(frac) = progress {
        progress_line(ui, full, frac);
    }

    let icons = cmds.min_rect();
    let empty = |p: egui::Pos2| !icons.contains(p) && !right.contains(p);
    move_or_maximize(ui, &bg, empty, maximized);
    if bg.secondary_clicked() {
        if let Some(p) = bg.interact_pointer_pos().filter(|&p| empty(p)) {
            window_menu(ui, p);
        }
    }
    act
}

/// The window manager's own window menu (keep above, move to desktop, …), where it offers one.
fn window_menu(ui: &Ui, pos: egui::Pos2) {
    #[cfg(target_os = "linux")]
    {
        let px = (pos.to_vec2() * ui.ctx().pixels_per_point()).to_pos2();
        crate::ui::x11_wm::show_window_menu(px);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (ui, pos);
}

/// While the search icon is hovered: a greyed-out search box where the title goes, to show
/// where search will live.
fn search_placeholder(ui: &mut Ui, mid: Rect, full: Rect) {
    let w = SEARCH_W.min(mid.width());
    if w < 40.0 {
        return;
    }
    let h = BUTTON.y - 4.0;
    let cx = full
        .center()
        .x
        .clamp(mid.left() + w / 2.0, mid.right() - w / 2.0);
    let rect = Rect::from_center_size(egui::pos2(cx, full.center().y), Vec2::new(w, h));
    let mut text = String::new();
    let edit = egui::TextEdit::singleline(&mut text)
        .hint_text(format!("{}  Search (coming later)", icon::MAGNIFYING_GLASS))
        .desired_width(w);
    ui.put(rect, |ui: &mut Ui| ui.add_enabled(false, edit));
}

/// A thin line along the bottom of the bar, `frac` of the way across.
fn progress_line(ui: &Ui, full: Rect, frac: f32) {
    let w = full.width() * frac.clamp(0.0, 1.0);
    let line = Rect::from_min_size(
        egui::pos2(full.left(), full.bottom() - PROGRESS_H),
        Vec2::new(w, PROGRESS_H),
    );
    ui.painter()
        .rect_filled(line, 0.0, ui.visuals().selection.bg_fill);
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

/// The icons left of the title. Also returns whether the search icon is hovered.
fn commands(ui: &mut Ui, st: &CommandState, keys: &Keymap) -> (Option<Action>, bool) {
    let mut act = main_menu(ui, st, keys);
    // Placeholders for zoom history (see TODO.md).
    icon_button(ui, icon::ARROW_LEFT, false, "Back (coming later)");
    icon_button(ui, icon::ARROW_RIGHT, false, "Forward (coming later)");
    divider(ui);
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
    let search = ui.add_enabled(
        false,
        Button::new(RichText::new(icon::MAGNIFYING_GLASS).size(ICON_SIZE))
            .frame_when_inactive(false)
            .min_size(BUTTON),
    );
    (act, search.contains_pointer())
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
