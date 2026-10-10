//! The treemap panel: pointer input, layout, painting, menu and tips.

use super::SpaceMonger;
use crate::constants::{INFOTIP_DELAY, WHEEL_ZOOM};
use crate::core::actions::Action;
use crate::core::layout::{self, hit_test};
use crate::core::model::EntryRef;
use crate::core::selection::{Marquee, Selection};
use crate::ui::painter::MapPainter;
use crate::ui::widgets::{context_menu, infotip};
use eframe::egui::{self, Modifiers, PointerButton, Response, Sense};
use std::time::Instant;

impl SpaceMonger {
    pub(super) fn treemap(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        let d = MapPainter::new(&painter, resp.rect.min, ppp, self.palette(), self.font.clone());
        let (w, h) = (resp.rect.width(), resp.rect.height());
        let mods = ui.input(|i| i.modifiers);

        // Camera: start fully zoomed out; on resize, hold the folder in view steady.
        let (vw, vh) = (w as f64, h as f64);
        if (vw, vh) != self.camera.view {
            self.finish_anim();
            let (camera, scene, items) = self.camera_ctx();
            camera.resized(&scene, items, vw, vh);
        }
        self.camera.ensure();
        self.step_anim();

        self.view_input(ui, &resp, &d, mods);
        self.rebuild_layout(w, h);
        self.update_marquee(&resp, &d);

        // Hover.
        let hit = resp
            .hover_pos()
            .and_then(|p| {
                let (x, y) = d.local(p);
                hit_test(&self.items, x, y)
            })
            .filter(|_| self.camera.anim.is_none() && !resp.dragged());
        if hit != self.hovered {
            self.hovered = hit;
            self.hover_since = Instant::now();
        }

        self.paint(&d, w, h);

        let mut act = self.clicks(&resp, &d, mods);
        act = act.or(context_menu(&resp, &self.command_state()));
        self.show_infotip(ui, &resp);

        if self.camera.anim.is_some() {
            ui.ctx().request_repaint();
        }
        act
    }

    /// Wheel / pinch zoom, Shift+drag rectangle select, drag to pan.
    fn view_input(&mut self, ui: &egui::Ui, resp: &Response, d: &MapPainter, mods: Modifiers) {
        // Wheel / pinch: zoom about the pointer, like an infinite canvas.
        if resp.hovered() && self.camera.anim.is_none() && self.marquee.is_none() && self.tree.is_some() {
            let (dy, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let k = pinch as f64 * (dy as f64 * WHEEL_ZOOM).exp();
            if (k - 1.0).abs() > 1e-6 {
                if let Some(p) = resp.hover_pos() {
                    let (x, y) = d.local(p);
                    let (camera, scene, items) = self.camera_ctx();
                    camera.zoom_at(&scene, items, x as f64, y as f64, k);
                }
            }
        }
        // Shift+drag (left button): rectangle select; with Ctrl too, add to the selection.
        if self.tree.is_some() && resp.drag_started_by(PointerButton::Primary) && mods.shift {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                self.finish_anim();
                let base = if mods.command { self.selection.clone() } else { Selection::default() };
                self.marquee = Some(Marquee::new(d.local(p), base));
            }
        }
        if self.marquee.is_some() && !resp.dragged_by(PointerButton::Primary) && !resp.drag_stopped() {
            self.marquee = None;
        }
        // Drag (left or middle button) to pan.
        if self.tree.is_some()
            && self.marquee.is_none()
            && (resp.dragged_by(PointerButton::Primary) || resp.dragged_by(PointerButton::Middle))
        {
            self.finish_anim();
            let delta = resp.drag_delta();
            let (camera, scene, _) = self.camera_ctx();
            camera.pan(&scene, delta.x as f64, delta.y as f64);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
    }

    /// Rebuild the layout when the view, data, camera or reshaping changes.
    fn rebuild_layout(&mut self, w: f32, h: f32) {
        let (cam, ovs) = {
            let (camera, scene, _) = self.camera_ctx();
            camera.view_state(&scene)
        };
        let key = (w, h, self.generation, cam, ovs);
        if self.layout_key.as_ref() != Some(&key) {
            let (vw, vh) = (w as f64, h as f64);
            let p = self.params();
            self.items = match &self.tree {
                Some(t) => layout::build(&t.root, cam, vw, vh, p, &key.4),
                None => Vec::new(),
            };
            self.zoom = layout::covering(&self.items, vw, vh);
            self.layout_key = Some(key);
            self.hovered = None;
        }
    }

    /// Live rectangle selection against this frame's layout.
    fn update_marquee(&mut self, resp: &Response, d: &MapPainter) {
        let Some(m) = &mut self.marquee else { return };
        let end = resp.interact_pointer_pos().map(|p| d.local(p)).unwrap_or(m.end);
        self.selection = m.selection(&self.items, end);
        m.end = end;
        if resp.drag_stopped() {
            self.marquee = None;
        }
    }

    fn paint(&self, d: &MapPainter, w: f32, h: f32) {
        let pal = self.palette();
        d.fill(pal.background, 0.0, 0.0, w, h);
        if let Some(tree) = &self.tree {
            // Parents come before children, so a selected folder's children stay visible.
            for (i, it) in self.items.iter().enumerate() {
                let is_sel = it.index.is_some_and(|k| self.selection.contains(&EntryRef::new(it.folder.clone(), k)));
                d.item(tree, it, is_sel, self.hovered == Some(i));
            }
        }
        if let Some(m) = &self.marquee {
            d.marquee(m.start, m.end);
        }
    }

    /// Click, right-click and double-click selection.
    fn clicks(&mut self, resp: &Response, d: &MapPainter, mods: Modifiers) -> Option<Action> {
        let pointer_hit = |items: &[layout::Item]| {
            resp.interact_pointer_pos().and_then(|p| {
                let (x, y) = d.local(p);
                hit_test(items, x, y)
            })
        };
        if resp.clicked() {
            let hit = pointer_hit(&self.items).and_then(|i| self.item_ref(i));
            match hit {
                // Ctrl+click toggles, Shift+click adds.
                Some(r) if mods.command => self.selection.toggle(r),
                Some(r) if mods.shift => self.selection.add(r),
                None if mods.command || mods.shift => {}
                // Plain click on the only selected item deselects it.
                Some(r) if self.selection.is_only(&r) => self.selection.clear(),
                r => self.selection.set(r),
            }
        }
        if resp.secondary_clicked() {
            // Right-clicking inside the selection keeps it, so the menu acts on all of it.
            let r = pointer_hit(&self.items).and_then(|i| self.item_ref(i));
            if !r.as_ref().is_some_and(|r| self.selection.contains(r)) {
                self.selection.set(r);
            }
        }
        if resp.double_clicked() && !mods.command {
            if let Some(i) = pointer_hit(&self.items) {
                self.selection.set(self.item_ref(i));
                return Some(if self.items[i].is_folder { Action::ZoomIn } else { Action::RunOpen });
            }
        }
        None
    }

    /// Name / size / date tip after hovering a box for a moment.
    fn show_infotip(&self, ui: &egui::Ui, resp: &Response) {
        let (Some(hi), Some(tree), false) = (self.hovered, &self.tree, resp.context_menu_opened()) else { return };
        let held = self.hover_since.elapsed();
        if held < INFOTIP_DELAY {
            ui.ctx().request_repaint_after(INFOTIP_DELAY - held);
            return;
        }
        let it = &self.items[hi];
        if let (Some(e), Some(pos)) = (it.index.and_then(|i| tree.entry_at(&it.folder, i)), resp.hover_pos()) {
            infotip(ui.ctx(), pos, e);
        }
    }
}

