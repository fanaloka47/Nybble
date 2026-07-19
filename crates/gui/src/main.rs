//! Nybble desktop GUI entry point.

// On Windows release builds, attach to the "windows" subsystem so launching the
// app doesn't pop up a console window behind it. Inert on other platforms, and
// left off in debug builds so stderr/PC_DEBUG output stays visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod changelog;
mod icon;
mod settings;
mod theme;
mod update;
mod widgets;

use app::App;

fn main() -> eframe::Result<()> {
    let icon = {
        let rgba = icon::icon_rgba(256);
        egui::IconData {
            rgba,
            width: 256,
            height: 256,
        }
    };
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
            // Floor matches the Compact preset's width (420) — the narrowest
            // intended layout — with a height where the content starts to scroll.
            // Below this the bit grid and settings modal get cramped.
            .with_min_inner_size([420.0, 460.0])
            .with_icon(icon)
            // Pinned, not derived from the window title: this string is what
            // eframe turns into the persistence path (`~/.local/share/nybble`,
            // `%APPDATA%\Nybble\data`), so letting it follow the title would
            // orphan everyone's saved settings the day the title changes. It
            // also has to match `StartupWMClass` in the Linux .desktop file for
            // the window to associate with its launcher icon.
            .with_app_id("Nybble"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native("Nybble", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
