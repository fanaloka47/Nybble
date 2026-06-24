//! Nybble desktop GUI entry point.

// On Windows release builds, attach to the "windows" subsystem so launching the
// app doesn't pop up a console window behind it. Inert on other platforms, and
// left off in debug builds so stderr/PC_DEBUG output stays visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod theme;
mod update;
mod widgets;

use app::App;

fn main() -> eframe::Result<()> {
    // `PC_SIZE=WIDTHxHEIGHT` overrides the initial window size, handy for
    // reproducing a layout bug at the exact size it was reported.
    let size = std::env::var("PC_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some([w.trim().parse().ok()?, h.trim().parse().ok()?])
        })
        .unwrap_or([760.0, 720.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([520.0, 480.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Nybble",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
