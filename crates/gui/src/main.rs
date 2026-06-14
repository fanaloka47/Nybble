//! PowerCalc desktop GUI entry point.

mod app;
mod theme;
mod widgets;

use app::App;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 720.0])
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
