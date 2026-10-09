//! SpaceMonger One — treemap disk space viewer. Rust port of spacemonger1 (Java).

mod app;
mod colors;
mod format;
mod layout;
mod scan;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let open_path = std::env::args_os().nth(1).map(PathBuf::from);
    if matches!(open_path.as_deref().and_then(|p| p.to_str()), Some("-h" | "--help")) {
        println!("usage: spacemonger [FOLDER]");
        return Ok(());
    }

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("SpaceMonger One")
        .with_app_id("spacemonger")
        .with_inner_size([1200.0, 800.0])
        .with_min_inner_size([400.0, 300.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/SpaceMonger.png")) {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "spacemonger",
        options,
        Box::new(move |cc| Ok(Box::new(app::SpaceMonger::new(cc, open_path)))),
    )
}
