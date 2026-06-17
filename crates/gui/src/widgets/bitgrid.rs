//! Interactive bit view: a popcount header, a visual bit bar, and a grid of
//! clickable 0/1 cells laid out MSB→LSB in non-breaking nibble groups.

use egui::{Color32, CornerRadius, Sense, Vec2};
use powercalc_core::Value;

const CELL: f32 = 26.0; // bit cell size (square-ish)
const CELL_GAP: f32 = 2.0; // gap between bits inside a nibble group
const GROUP_GAP: f32 = 6.0; // gap between nibble groups
const BAR_H: f32 = 13.0; // visual bit bar height
const BAR_NIBBLE_GAP: f32 = 4.0; // gap between nibbles in the bar

/// Render the bits of `value`: a header (MSB index · set count · LSB index), a
/// compact visual bar, and a grid of clickable 0/1 cells, most significant bit
/// first. Cells are packed into groups of four that never split across a row.
/// Set bits are filled with `accent`. If the user clicks a bit (in the bar or
/// the grid), returns the new value with that bit toggled; otherwise `None`.
pub fn bit_grid(ui: &mut egui::Ui, value: Value, accent: Color32) -> Option<Value> {
    let on_accent = contrast_text(accent);
    let w = value.width().bits() as i64;
    if w == 0 {
        return None;
    }
    let raw = value.raw();
    let mut toggled: Option<u32> = None;

    // Header: highest bit index on the left; set count and bit 0 on the right.
    // Kept on a single bounded row so nothing drifts vertically.
    let popcount = raw.count_ones();
    ui.horizontal(|ui| {
        ui.monospace(egui::RichText::new(format!("bit {}", w - 1)).weak().small());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.monospace(egui::RichText::new("bit 0").weak().small());
            ui.add_space(10.0);
            ui.monospace(
                egui::RichText::new(format!("{popcount} set"))
                    .small()
                    .color(accent),
            );
        });
    });
    ui.add_space(6.0);

    // Visual bit bar: one thin segment per bit, MSB→LSB, with a small gap on
    // nibble boundaries. The whole bar is clickable.
    if let Some(b) = bit_bar(ui, value, accent) {
        toggled = Some(b);
    }
    ui.add_space(10.0);

    // Grid of nibble groups. Pack bit indices MSB→LSB into chunks of four, then
    // lay out as many whole groups per row as fit.
    let groups: Vec<Vec<i64>> = (0..w)
        .rev()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|g| g.to_vec())
        .collect();

    // Each cell is allocated at exactly CELL×28 (via `add_sized`), so the group
    // width below is the real rendered width — otherwise button padding would
    // make cells wider than estimated and we'd pack one group too many per row.
    let group_w = 4.0 * CELL + 3.0 * CELL_GAP;
    let avail = ui.available_width();
    let per_row = (((avail + GROUP_GAP) / (group_w + GROUP_GAP)).floor() as usize).max(1);

    for row in groups.chunks(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GROUP_GAP;
            // Zero the button padding so each cell is exactly CELL wide: the
            // default padding would expand cells past the estimate used to
            // compute `per_row`, overflowing the column.
            ui.spacing_mut().button_padding = Vec2::ZERO;
            for group in row {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = CELL_GAP;
                    for &b in group {
                        let set = (raw >> b) & 1 == 1;
                        let text = egui::RichText::new(if set { "1" } else { "0" })
                            .monospace()
                            .color(if set { on_accent } else { ui.visuals().text_color() });
                        let mut btn = egui::Button::new(text).min_size(Vec2::new(CELL, 28.0));
                        if set {
                            btn = btn.fill(accent);
                        }
                        if ui.add(btn).on_hover_text(format!("bit {b}")).clicked() {
                            toggled = Some(b as u32);
                        }
                    }
                });
            }
        });
    }

    toggled.map(|b| value.with_raw(value.raw() ^ (1u128 << b)))
}

/// The thin visual bit bar. Returns the index of a clicked bit, if any.
fn bit_bar(ui: &mut egui::Ui, value: Value, accent: Color32) -> Option<u32> {
    let w = value.width().bits() as i64;
    let raw = value.raw();
    let avail = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail, BAR_H), Sense::click());

    // Lay bits out MSB→LSB with a gap between nibble groups.
    let groups = ((w + 3) / 4) as f32;
    let total_gap = (groups - 1.0).max(0.0) * BAR_NIBBLE_GAP;
    let per_bit = ((rect.width() - total_gap) / w as f32).max(1.0);

    let off = ui.visuals().widgets.inactive.bg_fill;
    let painter = ui.painter();
    let mut x = rect.left();
    let mut clicked: Option<u32> = None;
    let click_x = resp.clicked().then(|| resp.interact_pointer_pos()).flatten();

    let mut drawn = 0;
    for b in (0..w).rev() {
        let set = (raw >> b) & 1 == 1;
        let seg = egui::Rect::from_min_size(
            egui::pos2(x, rect.top()),
            Vec2::new(per_bit, rect.height()),
        );
        painter.rect_filled(seg, CornerRadius::ZERO, if set { accent } else { off });
        if let Some(p) = click_x {
            if p.x >= x && p.x < x + per_bit {
                clicked = Some(b as u32);
            }
        }
        x += per_bit;
        drawn += 1;
        if drawn % 4 == 0 && b != 0 {
            x += BAR_NIBBLE_GAP;
        }
    }
    clicked
}

/// Pick black or white text for legibility on top of `bg`.
fn contrast_text(bg: Color32) -> Color32 {
    let luminance = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if luminance > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}
