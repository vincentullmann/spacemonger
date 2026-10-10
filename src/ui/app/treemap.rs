//! The treemap panel: pointer input, layout, painting, menu and tips.

use super::SpaceMonger;
use crate::core::actions::Action;
use crate::core::geometry::{Point, Size, Vec2};
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
        let d = MapPainter::new(
            &painter,
            resp.rect.min,
            ppp,
            self.palette(ui.ctx()),
            self.settings.map_font(),
        );
        let (w, h) = (resp.rect.width(), resp.rect.height());
        let mods = ui.input(|i| i.modifiers);
        // A running scan's snapshot is only drawn; its entries move around as the scan goes.
        let interactive = self.tree.is_some();

        // Camera: start fully zoomed out; on resize, hold the folder in view steady.
        let view = Size::new(w as f64, h as f64);
        if view != self.camera.view {
            self.finish_anim();
            let (camera, scene, items) = self.camera_ctx();
            camera.resized(&scene, items, view);
        }
        self.camera.ensure();
        self.step_anim();

        if interactive {
            self.view_input(ui, &resp, &d, mods);
        }
        self.rebuild_layout(w, h);
        self.update_marquee(&resp, &d);

        // Hover.
        let title_h = self.settings.title_h();
        let hit = resp
            .hover_pos()
            .and_then(|p| {
                let (x, y) = d.local(p);
                hit_test(&self.items, x, y, title_h)
            })
            .filter(|_| interactive && self.camera.anim.is_none() && !resp.dragged());
        if hit != self.hovered {
            self.hovered = hit;
            self.hover_since = Instant::now();
        }

        self.paint(&d, w, h);

        if !interactive {
            return None;
        }
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
        if resp.hovered()
            && self.camera.anim.is_none()
            && self.marquee.is_none()
            && self.tree.is_some()
        {
            let (dy, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let k = pinch as f64 * (dy as f64 * self.settings.wheel_zoom()).exp();
            if (k - 1.0).abs() > 1e-6 {
                if let Some(p) = resp.hover_pos() {
                    let (x, y) = d.local(p);
                    let (camera, scene, items) = self.camera_ctx();
                    camera.zoom_at(&scene, items, Point::new(x as f64, y as f64), k);
                }
            }
        }
        // Shift+drag (left button): rectangle select; with Ctrl too, add to the selection.
        if self.tree.is_some() && resp.drag_started_by(PointerButton::Primary) && mods.shift {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                self.finish_anim();
                let base = if mods.command {
                    self.selection.clone()
                } else {
                    Selection::default()
                };
                self.marquee = Some(Marquee::new(d.local(p), base));
            }
        }
        if self.marquee.is_some()
            && !resp.dragged_by(PointerButton::Primary)
            && !resp.drag_stopped()
        {
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
            camera.pan(&scene, Vec2::new(delta.x as f64, delta.y as f64));
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
            let view = Size::new(w as f64, h as f64);
            let p = self.params();
            self.items = match self.shown_tree() {
                Some(t) => layout::build(&t.root, cam, view, p, &key.4),
                None => Vec::new(),
            };
            self.zoom = layout::covering(&self.items, view, p.title_h);
            self.layout_key = Some(key);
            self.hovered = None;
        }
    }

    /// Live rectangle selection against this frame's layout.
    fn update_marquee(&mut self, resp: &Response, d: &MapPainter) {
        let Some(m) = &mut self.marquee else { return };
        let end = resp
            .interact_pointer_pos()
            .map(|p| d.local(p))
            .unwrap_or(m.end);
        self.selection = m.selection(&self.items, end);
        m.end = end;
        if resp.drag_stopped() {
            self.marquee = None;
        }
    }

    fn paint(&self, d: &MapPainter, w: f32, h: f32) {
        d.fill(d.palette().background, 0.0, 0.0, w, h);
        if let Some(tree) = self.shown_tree() {
            // Parents come before children, so a selected folder's children stay visible.
            let is_sel = |it: &layout::Item| {
                it.index.is_some_and(|k| {
                    self.selection
                        .contains(&EntryRef::new(it.folder.clone(), k))
                })
            };
            for (i, it) in self.items.iter().enumerate() {
                d.item(tree, it, is_sel(it), self.hovered == Some(i));
            }
            // Collapsed borders: highlight outlines go on top of the neighbours' lines.
            if d.collapsed() {
                for (i, it) in self.items.iter().enumerate() {
                    let (sel, hover) = (is_sel(it), self.hovered == Some(i));
                    if (sel || hover) && !it.is_free {
                        d.outline(it, sel, hover);
                    }
                }
            }
        }
        if let Some(m) = &self.marquee {
            d.marquee(m.start, m.end);
        }
    }

    /// Click, right-click and double-click selection.
    fn clicks(&mut self, resp: &Response, d: &MapPainter, mods: Modifiers) -> Option<Action> {
        let title_h = self.settings.title_h();
        let pointer_hit = |items: &[layout::Item]| {
            resp.interact_pointer_pos().and_then(|p| {
                let (x, y) = d.local(p);
                hit_test(items, x, y, title_h)
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
                return Some(if self.items[i].is_folder {
                    Action::ZoomIn
                } else {
                    Action::RunOpen
                });
            }
        }
        None
    }

    /// Name / size / date tip after hovering a box for a moment.
    fn show_infotip(&self, ui: &egui::Ui, resp: &Response) {
        let (Some(hi), Some(tree), false) = (self.hovered, &self.tree, resp.context_menu_opened())
        else {
            return;
        };
        let (held, delay) = (self.hover_since.elapsed(), self.settings.infotip_delay());
        if held < delay {
            ui.ctx().request_repaint_after(delay - held);
            return;
        }
        let it = &self.items[hi];
        if let (Some(i), Some(pos)) = (it.index, resp.hover_pos()) {
            if let Some(e) = tree.entry_at(&it.folder, i) {
                let path = tree.full_path(&it.folder, Some(i));
                infotip(ui.ctx(), pos, e, &path, &self.settings.tooltips);
            }
        }
    }
}
