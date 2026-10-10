//! Camera tests on a small nested tree.

use super::*;
use crate::core::geometry::Rect;
use crate::core::layout::{content, root_content, LayoutParams, Scene};
use crate::core::model::{Entry, Folder, Kind};

const VW: f64 = 400.0;
const VH: f64 = 300.0;

fn file(name: &str, size: u64) -> Entry {
    Entry { name: name.into(), size, actual: size, mtime: 0, kind: Kind::File, hidden: false }
}

fn tree() -> Folder {
    let sub = Folder { entries: vec![file("x", 700), file("y", 300)], total: 1000 };
    Folder {
        entries: vec![
            Entry { name: "sub".into(), size: 1000, actual: 1000, mtime: 0, kind: Kind::Dir(Box::new(sub)), hidden: false },
            file("b", 500),
            file("c", 250),
        ],
        total: 1750,
    }
}

fn camera() -> Camera {
    let mut c = Camera { view: (VW, VH), ..Default::default() };
    c.ensure();
    c
}

fn close(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 0.5 && (a.y - b.y).abs() < 0.5 && (a.w - b.w).abs() < 0.5 && (a.h - b.h).abs() < 0.5
}

#[test]
fn fit_blend_fades_between_half_and_full_scale() {
    let f = Fit { reshape: crate::core::layout::Reshape { path: vec![0], a: (1.0, 1.0), d: (0.0, 0.0) }, scale: 4.0 };
    assert_eq!(f.blend(4.0), 1.0);
    assert_eq!(f.blend(8.0), 1.0);
    assert_eq!(f.blend(2.0), 0.0);
    assert!((f.blend(2f64.powf(1.5)) - 0.5).abs() < 1e-9);
}

#[test]
fn zoom_to_folder_fills_view_and_clears_selection() {
    let root = tree();
    let scene = Scene { root: Some(&root), params: LayoutParams::default() };
    let mut c = camera();
    assert!(!c.zoomed());
    c.zoom_to(&scene, &[0]);
    assert!(c.anim.is_some());
    assert!(c.finish_anim(), "zooming in asks to clear the selection");
    assert!(c.zoomed());
    let cam = c.cam.unwrap();
    let b = scene.locate(cam, &[0], &c.ovs_at(cam)).unwrap();
    assert!(close(content(b), root_content(c.full_view())), "folder content fills the view");

    // Fitted, so Zoom Out goes to the parent; zooming there is back to the full view.
    assert_eq!(c.zoom_out_target(&scene, &[0]), Some(vec![]));
    c.zoom_to(&scene, &[]);
    let _ = c.finish_anim();
    assert_eq!(c.cam, Some(c.full_view()));
    assert!(c.fit.is_none());
    assert!(!c.zoomed());
}

#[test]
fn framing_keeps_selection() {
    let root = tree();
    let scene = Scene { root: Some(&root), params: LayoutParams::default() };
    let mut c = camera();
    c.frame(&scene, &[vec![2]]);
    assert!(!c.finish_anim(), "framing keeps the selection");
    let b = scene.locate(c.cam.unwrap(), &[2], &[]).unwrap();
    assert!(b.x >= -0.5 && b.y >= -0.5 && b.x + b.w <= VW + 0.5 && b.y + b.h <= VH + 0.5);
}

#[test]
fn pan_and_zoom_keep_root_covering_view() {
    let root = tree();
    let scene = Scene { root: Some(&root), params: LayoutParams::default() };
    let mut c = camera();
    // Can't pan or zoom out past the root.
    c.pan(&scene, 50.0, 50.0);
    assert_eq!(c.cam, Some(c.full_view()));
    c.zoom_at(&scene, &[], 100.0, 100.0, 0.5);
    assert_eq!(c.cam, Some(c.full_view()));
    // Zoom in 2x about a point: that point stays put.
    c.zoom_at(&scene, &[], 100.0, 100.0, 2.0);
    let cam = c.cam.unwrap();
    assert!((cam.w - 2.0 * VW).abs() < 1e-9);
    assert!((cam.x + 100.0).abs() < 1e-9 && (cam.y + 100.0).abs() < 1e-9);
    // Panning is clamped to the root's edges.
    c.pan(&scene, 1e6, 1e6);
    assert_eq!((c.cam.unwrap().x, c.cam.unwrap().y), (0.0, 0.0));
}

#[test]
fn reveal_pans_box_into_view() {
    let root = tree();
    let scene = Scene { root: Some(&root), params: LayoutParams::default() };
    let on_screen = |c: &Camera| {
        let b = scene.locate(c.cam.unwrap(), &[2], &[]).unwrap();
        (b, b.x >= 0.0 && b.y >= 0.0 && b.x + b.w <= VW + 1e-9 && b.y + b.h <= VH + 1e-9)
    };
    // Fits in the view: pan it fully in.
    let mut c = camera();
    c.zoom_at(&scene, &[], 0.0, 0.0, 2.0);
    assert!(!on_screen(&c).1, "c starts off-screen");
    c.reveal(&scene, &[2]);
    assert!(on_screen(&c).1);
    // Bigger than the view: line up its top-left corner with the view's.
    let mut c = camera();
    c.zoom_at(&scene, &[], 0.0, 0.0, 4.0);
    c.reveal(&scene, &[2]);
    let (b, _) = on_screen(&c);
    assert_eq!((b.x, b.y), (0.0, 0.0));
}

#[test]
fn resize_when_not_zoomed_stays_full_view() {
    let root = tree();
    let scene = Scene { root: Some(&root), params: LayoutParams::default() };
    let mut c = camera();
    c.resized(&scene, &[], 800.0, 500.0);
    assert_eq!(c.cam, Some(Rect::new(0.0, 0.0, 800.0, 500.0)));
}

#[test]
fn child_boxes_skip_free_space() {
    let mut root = tree();
    root.entries.push(Entry { name: String::new(), size: 100, actual: 100, mtime: 0, kind: Kind::Free, hidden: false });
    let scene = Scene { root: Some(&root), params: LayoutParams::default() };
    let c = camera();
    let boxes = c.child_boxes(&scene, &[]);
    assert_eq!(boxes.len(), 3);
    assert!(boxes.iter().all(|b| !matches!(root.entries[b.0].kind, Kind::Free)));
    assert_eq!(c.child_boxes(&scene, &[0]).len(), 2);
}
