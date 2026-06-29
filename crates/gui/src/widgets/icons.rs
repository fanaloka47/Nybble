//! Drawn icon buttons and their paint helpers — no glyph-font dependency.

use crate::theme::{self, ThemeMode};

/// Drawn "copy" icon button: two overlapping rectangles, no font dependency.
pub fn copy_icon_button(ui: &mut egui::Ui) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    let w = h * 0.85;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let vis = ui.style().interact(&resp);
        let stroke = egui::Stroke::new(1.2, vis.fg_stroke.color);
        let corner = egui::CornerRadius::same(2);
        let bg = theme::card_fill(ui.ctx());

        let pw = w * 0.50;
        let ph = h * 0.55;
        let cx = rect.center().x;
        let cy = rect.center().y;

        // Back page (offset up-right)
        let back = egui::Rect::from_center_size(egui::pos2(cx + 2.5, cy - 2.0), egui::vec2(pw, ph));
        ui.painter()
            .rect_stroke(back, corner, stroke, egui::StrokeKind::Middle);

        // Front page (offset down-left), filled to occlude the back page.
        let front =
            egui::Rect::from_center_size(egui::pos2(cx - 1.5, cy + 2.0), egui::vec2(pw, ph));
        ui.painter().rect_filled(front, corner, bg);
        ui.painter()
            .rect_stroke(front, corner, stroke, egui::StrokeKind::Middle);
    }

    resp.on_hover_text("Copy")
}

/// Secondary icon button that sends the field's value into the expression box.
/// Drawn as a small rightward arrow, matching `copy_icon_button`'s footprint.
pub fn send_icon_button(ui: &mut egui::Ui) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    let w = h * 0.85;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let vis = ui.style().interact(&resp);
        let stroke = egui::Stroke::new(1.4, vis.fg_stroke.color);
        let painter = ui.painter();
        let cy = rect.center().y;
        let x0 = rect.center().x - w * 0.28;
        let x1 = rect.center().x + w * 0.28;
        let head = w * 0.20;

        // Shaft
        painter.line_segment([egui::pos2(x0, cy), egui::pos2(x1, cy)], stroke);
        // Arrowhead
        painter.line_segment(
            [egui::pos2(x1, cy), egui::pos2(x1 - head, cy - head)],
            stroke,
        );
        painter.line_segment(
            [egui::pos2(x1, cy), egui::pos2(x1 - head, cy + head)],
            stroke,
        );
    }

    resp.on_hover_text("Send to expression")
}

/// Gear icon button that opens the settings modal. Drawn in Rust (a ring with
/// radial teeth and a hub) so it needs no glyph font, matching the other icons.
pub fn settings_icon_button(ui: &mut egui::Ui) -> egui::Response {
    let size = ui.spacing().interact_size.y;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return resp;
    }

    let vis = ui.style().interact(&resp);
    let bg = vis.weak_bg_fill;
    let col = vis.fg_stroke.color;
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(6), bg);

    let c = rect.center();
    let stroke = egui::Stroke::new(1.4, col);
    let r = 4.2_f32;
    // Eight teeth radiating from the ring.
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let (sin, cos) = a.sin_cos();
        ui.painter().line_segment(
            [
                egui::pos2(c.x + cos * r, c.y + sin * r),
                egui::pos2(c.x + cos * (r + 2.0), c.y + sin * (r + 2.0)),
            ],
            stroke,
        );
    }
    ui.painter().circle_stroke(c, r, stroke);
    ui.painter().circle_filled(c, 1.6, col);

    resp.on_hover_text("Settings")
}

/// Small close (✕) button drawn as two strokes, no font dependency.
pub fn close_icon_button(ui: &mut egui::Ui) -> egui::Response {
    cross_button(ui, "Close")
}

fn cross_button(ui: &mut egui::Ui, tooltip: &str) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(h, h), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let col = ui.style().interact(&resp).fg_stroke.color;
        let stroke = egui::Stroke::new(1.4, col);
        let c = rect.center();
        let r = 4.0;
        ui.painter().line_segment(
            [egui::pos2(c.x - r, c.y - r), egui::pos2(c.x + r, c.y + r)],
            stroke,
        );
        ui.painter().line_segment(
            [egui::pos2(c.x - r, c.y + r), egui::pos2(c.x + r, c.y - r)],
            stroke,
        );
    }
    resp.on_hover_text(tooltip)
}

/// Small up/down reorder button drawn as a filled triangle (JetBrains Mono has
/// no arrow glyphs, so we paint it). When `enabled` is false it renders greyed
/// and ignores clicks.
pub fn triangle_button(ui: &mut egui::Ui, up: bool, enabled: bool) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    let w = h * 0.7;
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), sense);
    if ui.is_rect_visible(rect) {
        let col = if enabled {
            ui.style().interact(&resp).fg_stroke.color
        } else {
            ui.visuals().weak_text_color()
        };
        let c = rect.center();
        let r = 4.0;
        let pts = if up {
            vec![
                egui::pos2(c.x, c.y - r),
                egui::pos2(c.x - r, c.y + r),
                egui::pos2(c.x + r, c.y + r),
            ]
        } else {
            vec![
                egui::pos2(c.x, c.y + r),
                egui::pos2(c.x - r, c.y - r),
                egui::pos2(c.x + r, c.y - r),
            ]
        };
        ui.painter()
            .add(egui::Shape::convex_polygon(pts, col, egui::Stroke::NONE));
    }
    resp
}

/// Single icon button showing the current theme; cycles on click.
/// Returns the next `ThemeMode` if clicked.
pub fn theme_icon_toggle(ui: &mut egui::Ui, current: ThemeMode) -> Option<ThemeMode> {
    let size = ui.spacing().interact_size.y;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return None;
    }

    let vis = ui.style().interact(&resp);
    let bg = vis.weak_bg_fill;
    let col = vis.fg_stroke.color;
    let cr = egui::CornerRadius::same(6);
    ui.painter().rect_filled(rect, cr, bg);

    let c = rect.center();
    match current {
        ThemeMode::Dark => draw_moon(ui.painter(), c, col, bg),
        ThemeMode::Auto => draw_auto_icon(ui.painter(), rect, c, col),
        ThemeMode::Light => draw_sun(ui.painter(), c, col),
    }

    let tip = match current {
        ThemeMode::Dark => "Dark (click for Light)",
        ThemeMode::Auto => "System (click for Dark)",
        ThemeMode::Light => "Light (click for System)",
    };
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    if clicked {
        Some(current.next())
    } else {
        None
    }
}

fn draw_moon(p: &egui::Painter, c: egui::Pos2, col: egui::Color32, bg: egui::Color32) {
    p.circle_filled(c, 5.5, col);
    p.circle_filled(egui::pos2(c.x + 2.5, c.y - 2.5), 4.2, bg);
}

fn draw_auto_icon(p: &egui::Painter, slot: egui::Rect, c: egui::Pos2, col: egui::Color32) {
    let r = 5.5_f32;
    let left_half = egui::Rect::from_min_max(
        egui::pos2(slot.left(), slot.top()),
        egui::pos2(c.x + 0.5, slot.bottom()),
    );
    p.with_clip_rect(p.clip_rect().intersect(left_half))
        .circle_filled(c, r, col);
    p.circle_stroke(c, r, egui::Stroke::new(1.5, col));
}

fn draw_sun(p: &egui::Painter, c: egui::Pos2, col: egui::Color32) {
    p.circle_filled(c, 3.5, col);
    for i in 0..6 {
        let a = i as f32 * std::f32::consts::TAU / 6.0;
        let (sin, cos) = a.sin_cos();
        p.line_segment(
            [
                egui::pos2(c.x + cos * 5.0, c.y + sin * 5.0),
                egui::pos2(c.x + cos * 6.5, c.y + sin * 6.5),
            ],
            egui::Stroke::new(1.5, col),
        );
    }
}
