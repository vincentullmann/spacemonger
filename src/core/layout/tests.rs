//! Layout tests (build and queries together).

use super::*;
use crate::core::geometry::{Rect, Size};
use crate::core::model::{Entry, Folder, Kind};

fn file(name: &str, size: u64) -> Entry {
    Entry { name: name.into(), size, actual: size, mtime: 0, kind: Kind::File, hidden: false }
}

/// Rect from x, y, width, height.
fn xywh(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::from_origin_size((x, y), (w, h))
}

fn full(w: f64, h: f64) -> Rect {
    xywh(0.0, 0.0, w, h)
}

#[test]
fn child_boxes_match_layout() {
    let mut entries: Vec<Entry> = (0..9).map(|i| file(&format!("f{i}"), 900 - i * 90)).collect();
    entries[4].hidden = true;
    let f = Folder { entries, total: 0 };
    let p = LayoutParams::default();
    let view = full(2001.0, 1201.0);
    let items = build(&f, view, Size::new(2001.0, 1201.0), p, &[]);
    let boxes = child_boxes(&f, root_content(view), p);
    assert_eq!(boxes.iter().map(|b| b.0).collect::<Vec<_>>(), items.iter().filter_map(|it| it.index).collect::<Vec<_>>());
    assert!(!boxes.iter().any(|b| b.0 == 4));
}

#[test]
fn neighbour_picks_adjacent_top_left_aligned() {
    // A tall box on the left, two stacked on the right, one wide box below those.
    //  0 | 1
    //    | 2
    //    |----
    //    |  3
    let b = [
        (0, xywh(0.0, 0.0, 10.0, 30.0)),
        (1, xywh(10.0, 0.0, 10.0, 10.0)),
        (2, xywh(10.0, 10.0, 10.0, 10.0)),
        (3, xywh(10.0, 20.0, 10.0, 10.0)),
    ];
    let at = |i: usize| b[i].1;
    assert_eq!(neighbour(&b, at(0), Dir::Right), Some(1)); // larger -> smaller: top-aligned
    assert_eq!(neighbour(&b, at(2), Dir::Left), Some(0));
    assert_eq!(neighbour(&b, at(1), Dir::Down), Some(2));
    assert_eq!(neighbour(&b, at(3), Dir::Up), Some(2));
    assert_eq!(neighbour(&b, at(0), Dir::Left), None);
    assert_eq!(neighbour(&b, at(1), Dir::Up), None);
}

#[test]
fn areas_roughly_proportional() {
    let f = Folder {
        entries: vec![file("a", 600), file("b", 300), file("c", 100)],
        total: 1000,
    };
    let items = build(&f, full(1001.0, 501.0), Size::new(1001.0, 501.0), LayoutParams::default(), &[]);
    assert_eq!(items.len(), 3);
    let area = |n: usize| {
        let it = items.iter().find(|i| i.index == Some(n)).unwrap();
        (it.w * it.h) as f64
    };
    let total = 1000.0 * 500.0;
    assert!((area(0) / total - 0.6).abs() < 0.02);
    assert!((area(1) / total - 0.3).abs() < 0.02);
    assert!((area(2) / total - 0.1).abs() < 0.02);
    // tiles don't overlap and hit-testing finds them
    let a = items.iter().position(|i| i.index == Some(0)).unwrap();
    let it = &items[a];
    assert_eq!(hit_test(&items, it.x + it.w / 2.0, it.y + it.h / 2.0, 12.0), Some(a));
}

#[test]
fn small_boxes_are_items_without_labels() {
    let f = Folder {
        entries: vec![file("big", 9000), file("s1", 300), file("s2", 200)],
        total: 9500,
    };
    let items = build(&f, full(400.0, 300.0), Size::new(400.0, 300.0), LayoutParams::default(), &[]);
    let s1 = items.iter().position(|i| i.index == Some(1)).expect("small file laid out");
    assert!(!items[s1].labeled);
    let it = &items[s1];
    assert_eq!(hit_test(&items, it.x + it.w / 2.0, it.y + it.h / 2.0, 12.0), Some(s1));
}

fn nested() -> Folder {
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

#[test]
fn locate_matches_build_and_culls() {
    let f = nested();
    let p = LayoutParams::default();
    // Zoomed in 3x around the middle: some boxes fall outside the view.
    let cam = xywh(-400.0, -300.0, 1200.0, 900.0);
    let items = build(&f, cam, Size::new(400.0, 300.0), p, &[]);
    for it in items.iter().filter(|it| it.index.is_some()) {
        let mut path = it.folder.to_vec();
        path.push(it.index.unwrap());
        let b = locate(&f, cam, &path, p, &[]).unwrap();
        // Clipped boxes are inside the located (unclipped) one.
        assert!(b.x0 <= it.x as f64 + 0.01 && b.y0 <= it.y as f64 + 0.01);
        assert!(b.x1 >= (it.x + it.w) as f64 - 0.01);
    }
    assert!(items.iter().all(|it| it.x < 400.0 && it.y < 300.0 && it.x + it.w > 0.0 && it.y + it.h > 0.0));
}

#[test]
fn override_fills_view_and_draws_on_top() {
    let f = nested();
    let p = LayoutParams::default();
    let (vw, vh) = (400.0, 300.0);
    let view = Size::new(vw, vh);
    let cam = full(vw, vh);
    assert!(covering(&build(&f, cam, view, p, &[]), view, 12.0).is_empty());
    // Reshape "sub" so its content exactly fills the view.
    let n = locate(&f, cam, &[0], p, &[]).unwrap();
    let t = xywh(-3.0, -12.0, vw + 5.0, vh + 14.0);
    let ov = Reshape::between(vec![0], n, t);
    let b = locate(&f, cam, &[0], p, std::slice::from_ref(&ov)).unwrap();
    assert!((b.x0 - t.x0).abs() < 1e-9 && (b.width() - t.width()).abs() < 1e-9 && (b.height() - t.height()).abs() < 1e-9);
    let items = build(&f, cam, view, p, std::slice::from_ref(&ov));
    assert_eq!(covering(&items, view, 12.0), vec![0]);
    // The reshaped folder and its children come last and win hit tests.
    let x = items.iter().position(|it| it.index == Some(0) && it.folder.len() == 1).unwrap();
    assert_eq!(hit_test(&items, 200.0, 150.0, 12.0), Some(x));
    let sub = items.iter().position(|it| it.index == Some(0) && it.folder.is_empty()).unwrap();
    assert!(sub > items.iter().position(|it| it.index == Some(1) && it.folder.is_empty()).unwrap());
}
