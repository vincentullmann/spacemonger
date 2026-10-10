//! Breadcrumb bar: one treemap-style box per folder from the scan root to the current view.

use crate::constants::BAR_H;
use crate::core::model::Tree;
use crate::ui::painter::MapPainter;
use crate::ui::palette::Palette;
use eframe::egui::{self, FontId, Sense, Vec2};

/// Draw the bar for the folder at `zoom`. Returns the zoom path to go to when a parent is
/// clicked.
pub fn path_bar(
    ui: &mut egui::Ui,
    tree: &Tree,
    zoom: &[usize],
    pal: Palette,
    font: &FontId,
) -> Option<Vec<usize>> {
    // (label, zoom path, colour). Folders keep the colour they have in the treemap.
    let mut segs = vec![(
        tree.root_path.display().to_string(),
        Vec::new(),
        pal.background,
    )];
    let mut f = &tree.root;
    for (k, &i) in zoom.iter().enumerate() {
        let Some(e) = f.entries.get(i) else { break };
        segs.push((e.name.clone(), zoom[..=k].to_vec(), pal.depth(k as i32)));
        match e.child() {
            Some(c) => f = c,
            None => break,
        }
    }

    let (resp, painter) = ui.allocate_painter(
        Vec2::new(ui.available_width(), BAR_H as f32),
        Sense::click(),
    );
    let d = MapPainter::new(
        &painter,
        resp.rect.min,
        ui.ctx().pixels_per_point(),
        pal,
        font.clone(),
    );
    let w = resp.rect.width() as i32;
    d.fill(pal.background, 0.0, 0.0, w as f32, BAR_H as f32);

    let pad = 12;
    let widths: Vec<i32> = segs
        .iter()
        .map(|(l, _, _)| d.text_width(l).0 as i32 + pad)
        .collect();
    let ellipsis_w = d.text_width("…").0 as i32 + pad;
    // Too long? Drop parents from the left (behind a "…" box), always keeping the current folder.
    let mut first = 0;
    while first + 1 < segs.len() {
        let used: i32 =
            widths[first..].iter().sum::<i32>() + if first > 0 { ellipsis_w } else { 0 };
        if used <= w {
            break;
        }
        first += 1;
    }

    // Lay out boxes: (x, width, segment index or None for the ellipsis).
    let mut boxes = Vec::new();
    let mut x = 0;
    if first > 0 {
        boxes.push((x, ellipsis_w, None));
        x += ellipsis_w;
    }
    for (i, &sw) in widths.iter().enumerate().skip(first) {
        // The current folder takes the rest of the bar, like a treemap row.
        let sw = if i + 1 == segs.len() {
            (w - x).max(sw)
        } else {
            sw
        };
        boxes.push((x, sw, Some(i)));
        x += sw;
    }

    let pointer = resp.hover_pos().map(|p| d.local(p).0 as i32);
    let last = segs.len() - 1;
    let hovered = |bx: i32, bw: i32, seg: Option<usize>| {
        seg.is_some_and(|i| i < last) && pointer.is_some_and(|px| px >= bx && px < bx + bw)
    };
    let mut clicked = None;
    for &(bx, bw, seg) in &boxes {
        let hover = hovered(bx, bw, seg);
        let (label, color) = match seg {
            Some(i) => (segs[i].0.as_str(), segs[i].2),
            None => ("…", pal.background),
        };
        d.cell(bx as f32, 0.0, bw as f32, BAR_H as f32, color, hover, label);
        if hover && resp.clicked() {
            clicked = seg.map(|i| segs[i].1.clone());
        }
    }
    if boxes.iter().any(|&(bx, bw, seg)| hovered(bx, bw, seg)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    clicked
}
