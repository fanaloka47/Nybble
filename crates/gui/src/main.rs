//! Nybble desktop GUI entry point.

// On Windows release builds, attach to the "windows" subsystem so launching the
// app doesn't pop up a console window behind it. Inert on other platforms, and
// left off in debug builds so stderr/PC_DEBUG output stays visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod changelog;
mod settings;
mod theme;
mod update;
mod widgets;

use app::App;

// ---------------------------------------------------------------------------
// App icon — 2×2 bit-grid (checkerboard), rendered in pure Rust.
// Proportions mirror assets/icon.svg (512px design).
// ---------------------------------------------------------------------------

fn icon_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let bg: [u8; 3] = [15, 17, 21]; // #0F1115
    let fg: [u8; 3] = [129, 140, 248]; // indigo-400 #818CF8

    let offset = (88.0 / 512.0 * s).round() as i32;
    let cell = (160.0 / 512.0 * s).round() as i32;
    let gap = (16.0 / 512.0 * s).round() as i32;
    let r = 24.0 / 512.0 * s;
    let stroke = 10.0 / 512.0 * s;

    // (x, y, filled)
    let cells = [
        (offset, offset, true),
        (offset + cell + gap, offset, false),
        (offset, offset + cell + gap, false),
        (offset + cell + gap, offset + cell + gap, true),
    ];

    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for i in 0..(size * size) as usize {
        rgba[i * 4] = bg[0];
        rgba[i * 4 + 1] = bg[1];
        rgba[i * 4 + 2] = bg[2];
        rgba[i * 4 + 3] = 255;
    }

    for py in 0..size as i32 {
        for px in 0..size as i32 {
            for &(cx, cy, filled) in &cells {
                let lx = px - cx;
                let ly = py - cy;
                if lx < 0 || lx >= cell || ly < 0 || ly >= cell {
                    continue;
                }
                if !in_rrect(lx, ly, cell, cell, r) {
                    continue;
                }
                let idx = ((py * size as i32 + px) * 4) as usize;
                if filled {
                    rgba[idx] = fg[0];
                    rgba[idx + 1] = fg[1];
                    rgba[idx + 2] = fg[2];
                } else if on_rrect_stroke(lx, ly, cell, cell, r, stroke) {
                    rgba[idx] = lerp_u8(bg[0], fg[0], 0.35);
                    rgba[idx + 1] = lerp_u8(bg[1], fg[1], 0.35);
                    rgba[idx + 2] = lerp_u8(bg[2], fg[2], 0.35);
                }
            }
        }
    }
    rgba
}

fn in_rrect(lx: i32, ly: i32, w: i32, h: i32, r: f32) -> bool {
    let fx = lx as f32 + 0.5;
    let fy = ly as f32 + 0.5;
    let in_xc = fx < r || fx > w as f32 - r;
    let in_yc = fy < r || fy > h as f32 - r;
    if in_xc && in_yc {
        let cx = if fx < r { r } else { w as f32 - r };
        let cy = if fy < r { r } else { h as f32 - r };
        let dx = fx - cx;
        let dy = fy - cy;
        dx * dx + dy * dy <= r * r
    } else {
        true
    }
}

fn on_rrect_stroke(lx: i32, ly: i32, w: i32, h: i32, r: f32, stroke: f32) -> bool {
    if !in_rrect(lx, ly, w, h, r) {
        return false;
    }
    let ix = lx as f32 + 0.5 - stroke;
    let iy = ly as f32 + 0.5 - stroke;
    let iw = w as f32 - 2.0 * stroke;
    let ih = h as f32 - 2.0 * stroke;
    if iw <= 0.0 || ih <= 0.0 || ix < 0.0 || iy < 0.0 || ix >= iw || iy >= ih {
        return true;
    }
    let ir = (r - stroke).max(0.0);
    let in_xc = ix < ir || ix > iw - ir;
    let in_yc = iy < ir || iy > ih - ir;
    if in_xc && in_yc {
        let cx = if ix < ir { ir } else { iw - ir };
        let cy = if iy < ir { ir } else { ih - ir };
        let dx = ix - cx;
        let dy = iy - cy;
        dx * dx + dy * dy > ir * ir
    } else {
        false
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn main() -> eframe::Result<()> {
    let icon = {
        let rgba = icon_rgba(256);
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
            .with_icon(icon),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native("Nybble", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
