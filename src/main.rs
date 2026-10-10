//! SpaceMonger One — treemap disk space viewer. Rust port of spacemonger1 (Java).

use std::path::PathBuf;

fn main() -> eframe::Result {
    let open_path = std::env::args_os().nth(1).map(PathBuf::from);
    if matches!(
        open_path.as_deref().and_then(|p| p.to_str()),
        Some("-h" | "--help")
    ) {
        println!("usage: spacemonger [FOLDER]");
        return Ok(());
    }

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title(spacemonger::constants::APP_NAME)
        .with_app_id("spacemonger")
        .with_inner_size([1200.0, 800.0])
        .with_min_inner_size([400.0, 300.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/SpaceMonger.png"))
    {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        // Linux: OpenGL. With wgpu (Vulkan) the swapchain goes `Outdated` on every size step of
        // a drag-resize, so eframe drops that frame and the window only catches up once the
        // drag pauses. GL just draws at the current size, so the treemap follows the drag.
        #[cfg(target_os = "linux")]
        renderer: eframe::Renderer::Glow,
        glow_options: eframe::egui_glow::GlowConfiguration {
            vsync: false,
            ..Default::default()
        },
        // We only repaint on input, so vsync buys nothing; without it a resize
        // never blocks waiting for a frame and the treemap redraws live.
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            surface: eframe::egui_wgpu::SurfaceConfig {
                present_mode: eframe::wgpu::PresentMode::AutoNoVsync,
                desired_maximum_frame_latency: Some(1),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "spacemonger",
        options,
        Box::new(move |cc| {
            Ok(Box::new(spacemonger::ui::app::SpaceMonger::new(
                cc, open_path,
            )))
        }),
    )
}
