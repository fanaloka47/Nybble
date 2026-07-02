//! App icon — a colored abacus, rendered in pure Rust (anti-aliased raw RGBA).
//!
//! Everything is laid out in a 512px design space and scaled to the requested
//! `size`; shapes are drawn with signed-distance coverage so edges stay crisp
//! at any size. Proportions mirror `assets/icon.svg`.

const BG: [u8; 3] = [15, 17, 21]; // #0F1115
const FRAME: [u8; 3] = [129, 140, 248]; // indigo-400 #818CF8
const ROD: [u8; 3] = [72, 82, 140]; // dim indigo rail

// Vivid bead palette, mixed across the rows.
const INDIGO: [u8; 3] = [129, 140, 248]; // #818CF8
const AMBER: [u8; 3] = [251, 191, 36]; // #FBBF24
const EMERALD: [u8; 3] = [52, 211, 153]; // #34D399
const ROSE: [u8; 3] = [251, 113, 133]; // #FB7185

/// Render the app icon as an RGBA8 buffer of `size`×`size` pixels.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let n = size as usize;
    let scale = size as f32 / 512.0;
    let sc = |v: f32| v * scale;

    let mut buf = vec![0u8; n * n * 4];
    for i in 0..n * n {
        buf[i * 4] = BG[0];
        buf[i * 4 + 1] = BG[1];
        buf[i * 4 + 2] = BG[2];
        buf[i * 4 + 3] = 255;
    }

    // Frame: rounded-rect border centered in the canvas.
    let frame = RRect {
        cx: sc(256.0),
        cy: sc(256.0),
        hw: sc(196.0),
        hh: sc(168.0),
        r: sc(30.0),
    };
    draw_rrect_stroke(&mut buf, n, frame, sc(22.0), FRAME);

    // Three rails, three beads each. The colors are mixed across the rows
    // rather than one hue per rail; `left` beads slide to the rail's left and
    // the rest to the right, leaving the classic abacus gap. Bead colors are
    // listed in visual left-to-right order.
    let rod_x0 = sc(82.0);
    let rod_x1 = sc(430.0);
    let radius = sc(36.0);
    let step = sc(80.0);
    let l_start = sc(124.0); // leftmost bead center
    let r_start = sc(388.0); // rightmost bead center
    let rows = [
        (sc(166.0), 1usize, [INDIGO, AMBER, ROSE]),
        (sc(256.0), 2, [EMERALD, ROSE, INDIGO]),
        (sc(346.0), 1, [AMBER, EMERALD, ROSE]),
    ];

    for &(y, left, colors) in &rows {
        // Rail (thin capsule) behind the beads.
        let rail = RRect {
            cx: (rod_x0 + rod_x1) * 0.5,
            cy: y,
            hw: (rod_x1 - rod_x0) * 0.5,
            hh: sc(4.0),
            r: sc(4.0),
        };
        draw_rrect_fill(&mut buf, n, rail, ROD);

        // Bead centers in visual (left-to-right) order, matching `colors`.
        let mut xs = [0.0f32; 3];
        for (i, x) in xs[..left].iter_mut().enumerate() {
            *x = l_start + step * i as f32;
        }
        for (i, x) in xs[left..].iter_mut().enumerate() {
            *x = r_start - step * i as f32;
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (&x, &color) in xs.iter().zip(colors.iter()) {
            draw_bead(&mut buf, n, x, y, radius, color);
        }
    }

    buf
}

/// A glossy bead: dark outline for separation, colored body, soft highlight.
fn draw_bead(buf: &mut [u8], n: usize, cx: f32, cy: f32, r: f32, color: [u8; 3]) {
    draw_circle(buf, n, cx, cy, r + r * 0.06 + 1.0, BG, 1.0);
    draw_circle(buf, n, cx, cy, r, color, 1.0);
    draw_circle(
        buf,
        n,
        cx - r * 0.3,
        cy - r * 0.3,
        r * 0.34,
        [255, 255, 255],
        0.45,
    );
}

/// A rounded rectangle by center, half-extents, and corner radius.
#[derive(Clone, Copy)]
struct RRect {
    cx: f32,
    cy: f32,
    hw: f32,
    hh: f32,
    r: f32,
}

/// Signed distance from `(px, py)` to the rounded rectangle `rr`.
fn rrect_sd(px: f32, py: f32, rr: RRect) -> f32 {
    let qx = (px - rr.cx).abs() - (rr.hw - rr.r);
    let qy = (py - rr.cy).abs() - (rr.hh - rr.r);
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - rr.r
}

fn blend(buf: &mut [u8], idx: usize, color: [u8; 3], a: f32) {
    let a = a.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    for k in 0..3 {
        let dst = buf[idx + k] as f32;
        buf[idx + k] = (dst + (color[k] as f32 - dst) * a).round() as u8;
    }
}

fn draw_circle(buf: &mut [u8], n: usize, cx: f32, cy: f32, r: f32, color: [u8; 3], alpha: f32) {
    let x0 = ((cx - r - 1.0).floor()).max(0.0) as usize;
    let x1 = ((cx + r + 1.0).ceil()).min(n as f32) as usize;
    let y0 = ((cy - r - 1.0).floor()).max(0.0) as usize;
    let y1 = ((cy + r + 1.0).ceil()).min(n as f32) as usize;
    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt() - r;
            let cov = (0.5 - d).clamp(0.0, 1.0);
            blend(buf, (py * n + px) * 4, color, cov * alpha);
        }
    }
}

fn draw_rrect_fill(buf: &mut [u8], n: usize, rr: RRect, color: [u8; 3]) {
    let x0 = ((rr.cx - rr.hw - 1.0).floor()).max(0.0) as usize;
    let x1 = ((rr.cx + rr.hw + 1.0).ceil()).min(n as f32) as usize;
    let y0 = ((rr.cy - rr.hh - 1.0).floor()).max(0.0) as usize;
    let y1 = ((rr.cy + rr.hh + 1.0).ceil()).min(n as f32) as usize;
    for py in y0..y1 {
        for px in x0..x1 {
            let d = rrect_sd(px as f32 + 0.5, py as f32 + 0.5, rr);
            let cov = (0.5 - d).clamp(0.0, 1.0);
            blend(buf, (py * n + px) * 4, color, cov);
        }
    }
}

fn draw_rrect_stroke(buf: &mut [u8], n: usize, rr: RRect, stroke: f32, color: [u8; 3]) {
    let x0 = ((rr.cx - rr.hw - 1.0).floor()).max(0.0) as usize;
    let x1 = ((rr.cx + rr.hw + 1.0).ceil()).min(n as f32) as usize;
    let y0 = ((rr.cy - rr.hh - 1.0).floor()).max(0.0) as usize;
    let y1 = ((rr.cy + rr.hh + 1.0).ceil()).min(n as f32) as usize;
    for py in y0..y1 {
        for px in x0..x1 {
            let d = rrect_sd(px as f32 + 0.5, py as f32 + 0.5, rr);
            let cov = (0.5 - (d.abs() - stroke * 0.5)).clamp(0.0, 1.0);
            blend(buf, (py * n + px) * 4, color, cov);
        }
    }
}
