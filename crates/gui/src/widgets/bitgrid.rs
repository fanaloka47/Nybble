//! Interactive bit grid: clickable toggle buttons laid out MSB→LSB.

use egui::Color32;
use powercalc_core::Value;

/// Render the bits of `value` as a grid of clickable 0/1 buttons, most
/// significant bit first, in rows of 8 with a nibble gap every 4 bits. Set bits
/// are filled with `accent`. Each row is prefixed with the index of its highest
/// bit. If the user clicks a bit, returns the new value with that bit toggled;
/// otherwise `None`.
pub fn bit_grid(ui: &mut egui::Ui, value: Value, accent: Color32) -> Option<Value> {
    let on_accent = contrast_text(accent);
    let w = value.width().bits() as i64;
    let mut toggled: Option<u32> = None;

    // Iterate rows in plain code (no closures) so the inner row closure only
    // needs to capture `toggled` mutably — avoids nested mutable-capture errors.
    let mut top = w - 1;
    while top >= 0 {
        let low = (top - 7).max(0);
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            ui.monospace(egui::RichText::new(format!("{top:>3}")).weak());
            ui.add_space(4.0);
            let mut b = top;
            while b >= low {
                let set = (value.raw() >> b) & 1 == 1;
                let text = egui::RichText::new(if set { "1" } else { "0" })
                    .monospace()
                    .color(if set { on_accent } else { ui.visuals().text_color() });
                let mut btn = egui::Button::new(text).min_size(egui::vec2(22.0, 22.0));
                if set {
                    btn = btn.fill(accent);
                }
                if ui.add(btn).clicked() {
                    toggled = Some(b as u32);
                }
                // Nibble gap: space after each group of 4 bits.
                if b % 4 == 0 && b != low {
                    ui.add_space(7.0);
                }
                b -= 1;
            }
        });
        top = low - 1;
    }

    toggled.map(|b| value.with_raw(value.raw() ^ (1u128 << b)))
}

/// Pick black or white text for legibility on top of `bg`.
fn contrast_text(bg: Color32) -> Color32 {
    let luminance =
        0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if luminance > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}
