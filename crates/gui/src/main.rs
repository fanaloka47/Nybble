//! PowerCalc desktop GUI entry point.

mod app;
mod theme;
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
        "PowerCalc",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
