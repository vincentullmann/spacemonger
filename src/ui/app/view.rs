//! Camera and selection moves: thin wrappers that hand the core the state it needs.

use super::SpaceMonger;
use crate::core::camera::Camera;
use crate::core::layout::{Item, Scene};
use crate::core::model::EntryRef;
use crate::core::selection::Nav;

impl SpaceMonger {
    /// The camera, plus what it needs to look at: the layout scene and this frame's items.
    pub(super) fn camera_ctx(&mut self) -> (&mut Camera, Scene<'_>, &[Item]) {
        let params = self.params();
        let scene = Scene::new(self.tree.as_ref(), params);
        (&mut self.camera, scene, &self.items)
    }

    /// End the camera move in progress, clearing the selection if it was a zoom-in.
    pub(super) fn finish_anim(&mut self) {
        if self.camera.finish_anim() {
            self.selection.clear();
        }
    }

    pub(super) fn step_anim(&mut self) {
        if self.camera.anim.as_ref().is_some_and(|a| a.done()) {
            self.finish_anim();
        }
    }

    /// Animate to the folder at `path`, filling the view.
    pub(super) fn zoom_to(&mut self, path: &[usize]) {
        self.finish_anim();
        let (camera, scene, _) = self.camera_ctx();
        camera.zoom_to(&scene, path);
    }

    /// Frame the selection (F): a single folder fills the view as with Zoom In; anything else
    /// is framed by its bounding box. Nothing selected frames the whole map. The selection stays.
    pub(super) fn frame_selection(&mut self) {
        let roots = self.selection.roots();
        let folder = |r: &EntryRef| {
            self.tree
                .as_ref()
                .and_then(|t| t.entry(r))
                .is_some_and(|e| e.child().is_some())
        };
        let single_folder = matches!(roots.as_slice(), [r] if folder(r));
        let paths: Vec<Vec<usize>> = roots.iter().map(EntryRef::path).collect();
        self.finish_anim();
        let (camera, scene, _) = self.camera_ctx();
        match paths.as_slice() {
            [] => camera.zoom_to(&scene, &[]),
            [p] if single_folder => camera.zoom_to(&scene, p),
            _ => camera.frame(&scene, &paths),
        }
        camera.keep_selection();
    }

    /// Zoom into the item at `idx` if it's a folder.
    pub(super) fn zoom_in_item(&mut self, idx: usize) {
        let it = &self.items[idx];
        let Some(target) = it.path().filter(|_| it.is_folder) else {
            return;
        };
        self.zoom_to(&target);
    }

    /// Zoom out: fit the current folder if it isn't already, otherwise its parent.
    pub(super) fn zoom_out(&mut self) {
        let zoom = self.zoom.clone();
        let (camera, scene, _) = self.camera_ctx();
        if let Some(target) = camera.zoom_out_target(&scene, &zoom) {
            self.zoom_to(&target);
        }
    }

    /// Arrow keys: move the selection (see `Selection::navigate`), then pan so the primary
    /// selection is in view.
    pub(super) fn navigate(&mut self, nav: Nav) {
        self.finish_anim();
        let mut sel = std::mem::take(&mut self.selection);
        let scene = Scene::new(self.tree.as_ref(), self.params());
        sel.navigate(nav, |f| self.camera.child_boxes(&scene, f));
        self.selection = sel;
        self.reveal_primary();
    }

    /// Pan so the primary selection is in view.
    fn reveal_primary(&mut self) {
        let Some(path) = self.selection.primary().map(EntryRef::path) else {
            return;
        };
        let (camera, scene, _) = self.camera_ctx();
        camera.reveal(&scene, &path);
    }
}
