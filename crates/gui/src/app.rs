//! PowerCalc application state and UI.
//!
//! The expression field is the centerpiece: type an expression, evaluate it, and
//! it becomes the current value *and* an entry in the history. One canonical
//! [`Value`] drives everything — the live base fields, the bit grid, and the
//! history all read from it. Signedness only changes the decimal rendering and
//! the meaning of `>>` and `/`.

use powercalc_core::{eval, fixed, Signedness, Value, Width};

use crate::theme::{self, ThemeMode};
use crate::widgets;

/// The editable surfaces that show the current value. Used to skip refreshing
/// the field the user is actively typing into.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Hex,
    Dec,
    Bin,
    Oct,
    Fixed,
}

/// Which base(s) the history list shows for each result.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum HistoryBase {
    #[default]
    All,
    Hex,
    Dec,
    Bin,
    Oct,
}

impl HistoryBase {
    const ALL: [HistoryBase; 5] = [
        HistoryBase::All,
        HistoryBase::Hex,
        HistoryBase::Dec,
        HistoryBase::Bin,
        HistoryBase::Oct,
    ];

    fn label(self) -> &'static str {
        match self {
            HistoryBase::All => "All",
            HistoryBase::Hex => "HEX",
            HistoryBase::Dec => "DEC",
            HistoryBase::Bin => "BIN",
            HistoryBase::Oct => "OCT",
        }
    }

    fn key(self) -> &'static str {
        match self {
            HistoryBase::All => "all",
            HistoryBase::Hex => "hex",
            HistoryBase::Dec => "dec",
            HistoryBase::Bin => "bin",
            HistoryBase::Oct => "oct",
        }
    }

    fn from_key(s: &str) -> Option<HistoryBase> {
        match s {
            "all" => Some(HistoryBase::All),
            "hex" => Some(HistoryBase::Hex),
            "dec" => Some(HistoryBase::Dec),
            "bin" => Some(HistoryBase::Bin),
            "oct" => Some(HistoryBase::Oct),
            _ => None,
        }
    }
}

/// One evaluated expression and its result, captured with the sign mode in
/// effect at the time so the decimal rendering stays faithful.
#[derive(Clone)]
struct HistoryEntry {
    expr: String,
    value: Value,
    sign: Signedness,
}

pub struct App {
    value: Value,
    width: Width,
    sign: Signedness,
    frac_bits: u32,

    // Text buffers backing the editable fields.
    hex: String,
    dec: String,
    bin: String,
    oct: String,
    fixed_input: String,
    expr: String,

    /// Which base is shown large and editable in the compact current-value view.
    focus_base: Field,

    history: Vec<HistoryEntry>,
    history_base: HistoryBase,

    /// Error from the last expression evaluation, shown inline under the field.
    /// Only set at evaluate time; cleared when the expression is edited.
    expr_error: Option<String>,

    /// Transient bottom-of-screen toast (copies, parse errors).
    status: Option<String>,
    /// `input.time` after which a transient toast auto-dismisses. Parse errors
    /// use [`f64::INFINITY`] so they persist until the next successful action.
    status_until: f64,

    /// `bits[hi:lo]` extraction range.
    range_hi: u32,
    range_lo: u32,
    /// Sub-pixel accumulator for the width drag-scrubber (3px per bit).
    width_scrub_accum: f32,

    theme_mode: ThemeMode,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let storage = cc.storage;
        let theme_mode = storage
            .and_then(|s| s.get_string("theme_mode"))
            .and_then(|s| ThemeMode::from_key(&s))
            .unwrap_or_default();
        let history_base = storage
            .and_then(|s| s.get_string("history_base"))
            .and_then(|s| HistoryBase::from_key(&s))
            .unwrap_or_default();

        let width = Width::new(32).unwrap();
        let mut app = Self {
            value: Value::new(0, width),
            width,
            sign: Signedness::Unsigned,
            frac_bits: 0,
            hex: String::new(),
            dec: String::new(),
            bin: String::new(),
            oct: String::new(),
            fixed_input: String::new(),
            expr: String::new(),
            focus_base: Field::Hex,
            history: Vec::new(),
            history_base,
            expr_error: None,
            status: None,
            status_until: 0.0,
            range_hi: 7,
            range_lo: 0,
            width_scrub_accum: 0.0,
            theme_mode,
        };
        app.refresh(None);
        app
    }

    /// Wrap a section's contents in a rounded, filled "card".
    fn section(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
        let fill = theme::card_fill(ui.ctx());
        egui::Frame::group(ui.style())
            .fill(fill)
            .inner_margin(egui::Margin::same(12))
            .corner_radius(egui::CornerRadius::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                add(ui);
            });
        ui.add_space(10.0);
    }

    // --- State updates ---------------------------------------------------

    /// Rewrite every field buffer from the canonical value, except `skip`.
    fn refresh(&mut self, skip: Option<Field>) {
        if skip != Some(Field::Hex) {
            self.hex = self.value.to_hex();
        }
        if skip != Some(Field::Dec) {
            self.dec = self.value.to_dec(self.sign);
        }
        if skip != Some(Field::Bin) {
            self.bin = self.value.to_bin();
        }
        if skip != Some(Field::Oct) {
            self.oct = self.value.to_oct();
        }
        if skip != Some(Field::Fixed) {
            self.fixed_input = self.format_fixed();
        }
    }

    fn format_fixed(&self) -> String {
        format!("{}", fixed::to_real(self.value, self.frac_bits, self.sign))
    }

    /// Copy `text` to the clipboard and flash a short, auto-dismissing toast.
    fn copy(&mut self, ctx: &egui::Context, text: String, label: &str) {
        ctx.copy_text(text);
        self.status = Some(format!("Copied {label}"));
        self.status_until = ctx.input(|i| i.time) + 1.4;
    }

    /// Show a persistent error toast (cleared on the next successful action).
    fn error(&mut self, msg: impl Into<String>) {
        self.status = Some(msg.into());
        self.status_until = f64::INFINITY;
    }

    fn set_width(&mut self, bits: u32) {
        self.width = Width::clamped(bits);
        self.value = self.value.with_width(self.width);
        if self.frac_bits > self.width.bits() {
            self.frac_bits = self.width.bits();
        }
        self.refresh(None);
    }

    fn set_sign(&mut self, sign: Signedness) {
        self.sign = sign;
        self.refresh(None);
    }

    /// Parse the buffer for `field`, update the value, and refresh the others.
    fn on_field_edit(&mut self, field: Field) {
        let (text, radix) = match field {
            Field::Hex => (self.hex.clone(), 16),
            Field::Dec => (self.dec.clone(), 10),
            Field::Bin => (self.bin.clone(), 2),
            Field::Oct => (self.oct.clone(), 8),
            Field::Fixed => unreachable!("fixed field handled separately"),
        };
        match parse_base(&text, radix, self.width, self.sign) {
            Ok(v) => {
                self.value = v;
                self.status = None;
                self.refresh(Some(field));
            }
            Err(e) => self.error(e),
        }
    }

    fn on_fixed_edit(&mut self) {
        let text = self.fixed_input.trim();
        if text.is_empty() {
            return;
        }
        match text.parse::<f64>() {
            Ok(real) => {
                self.value = fixed::from_real(real, self.width, self.frac_bits);
                self.status = None;
                self.refresh(Some(Field::Fixed));
            }
            Err(_) => self.error("invalid real number"),
        }
    }

    fn eval_expr(&mut self) {
        let trimmed = self.expr.trim().to_owned();
        if trimmed.is_empty() {
            return;
        }
        match eval(&self.expr, self.width, self.sign, self.value) {
            Ok(v) => {
                self.value = v;
                self.expr_error = None;
                self.status = None;
                self.push_history(trimmed, v);
                self.refresh(None);
            }
            Err(e) => self.expr_error = Some(format!("Invalid expression: {e}")),
        }
    }

    fn push_history(&mut self, expr: String, value: Value) {
        self.history.push(HistoryEntry {
            expr,
            value,
            sign: self.sign,
        });
        const MAX_HISTORY: usize = 200;
        if self.history.len() > MAX_HISTORY {
            let excess = self.history.len() - MAX_HISTORY;
            self.history.drain(0..excess);
        }
    }

    /// Bring a history entry back: restore its value, width, sign, and text.
    fn recall(&mut self, entry: HistoryEntry) {
        self.value = entry.value;
        self.width = entry.value.width();
        self.sign = entry.sign;
        if self.frac_bits > self.width.bits() {
            self.frac_bits = self.width.bits();
        }
        self.expr = entry.expr;
        self.expr_error = None;
        self.status = None;
        self.refresh(None);
    }

    // --- UI sections -----------------------------------------------------

    fn expression_centerpiece(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "EXPRESSION");

        let accent = theme::accent(ui.ctx());
        let on_accent = theme::on_accent(ui.ctx());

        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.expr)
                    .font(egui::FontId::new(22.0, egui::FontFamily::Monospace))
                    .desired_width(ui.available_width() - 112.0)
                    .hint_text("0xFF & (1 << 3)")
                    .margin(egui::vec2(10.0, 8.0)),
            );
            // Editing the expression clears any stale "invalid" message — we
            // only validate at evaluate time, never while typing.
            if resp.changed() {
                self.expr_error = None;
            }
            let entered =
                resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let clicked = ui
                .add_sized(
                    [96.0, 40.0],
                    egui::Button::new(
                        egui::RichText::new("Evaluate").size(16.0).color(on_accent),
                    )
                    .fill(accent),
                )
                .clicked();
            if clicked || entered {
                self.eval_expr();
                resp.request_focus();
            }
        });

        if let Some(err) = &self.expr_error {
            ui.add_space(6.0);
            ui.colored_label(egui::Color32::from_rgb(229, 115, 115), err);
        }
    }

    fn history_panel(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(egui::RichText::new("HISTORY").strong())
            .default_open(true)
            .show_unindented(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Clear").clicked() {
                            self.history.clear();
                        }
                        ui.separator();
                        for base in HistoryBase::ALL.into_iter().rev() {
                            if ui
                                .selectable_label(self.history_base == base, base.label())
                                .clicked()
                            {
                                self.history_base = base;
                            }
                        }
                    });
                });
                ui.add_space(4.0);
                self.history_list(ui);
            });
    }

    fn history_list(&mut self, ui: &mut egui::Ui) {
        if self.history.is_empty() {
            ui.label(
                egui::RichText::new("No expressions evaluated yet — type one above and press Enter.")
                    .weak(),
            );
            return;
        }

        let base = self.history_base;
        let accent = theme::accent(ui.ctx());
        let item_fill = ui.visuals().faint_bg_color;
        let mut recall_idx: Option<usize> = None;
        let mut copied: Option<&'static str> = None;

        egui::ScrollArea::vertical()
            .max_height(240.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Newest first.
                for (i, entry) in self.history.iter().enumerate().rev() {
                    egui::Frame::group(ui.style())
                        .fill(item_fill)
                        .stroke(egui::Stroke::NONE)
                        .inner_margin(egui::Margin::same(8))
                        .corner_radius(egui::CornerRadius::same(8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let expr = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&entry.expr).monospace().color(accent),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if expr.clicked() {
                                recall_idx = Some(i);
                            }
                            expr.on_hover_text("Click to recall this expression");
                            if let Some(label) = value_lines(ui, entry.value, entry.sign, base) {
                                copied = Some(label);
                            }
                        });
                    ui.add_space(6.0);
                }
            });

        if let Some(i) = recall_idx {
            let entry = self.history[i].clone();
            self.recall(entry);
        }
        if let Some(label) = copied {
            self.status = Some(format!("Copied {label}"));
            self.status_until = ui.ctx().input(|i| i.time) + 1.4;
        }
    }

    /// The FORMAT card: width (preset chips + a drag-scrubber), sign, the
    /// fixed-point split, and the bit-range extractor.
    fn format_section(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "FORMAT");
        let accent = theme::accent(ui.ctx());

        // Width: presets, then a draggable "{n}-bit" scrubber (3px per bit).
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Width").weak());
            for bits in [8u32, 16, 32, 64] {
                if ui
                    .selectable_label(self.width.bits() == bits, bits.to_string())
                    .clicked()
                {
                    self.set_width(bits);
                }
            }
            let scrub = ui
                .add(
                    egui::Label::new(
                        egui::RichText::new(format!("{}-bit ↔", self.width.bits()))
                            .monospace()
                            .color(accent),
                    )
                    .sense(egui::Sense::drag()),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
                .on_hover_text("Drag left / right to adjust width");
            if scrub.drag_started() {
                self.width_scrub_accum = 0.0;
            }
            if scrub.dragged() {
                self.width_scrub_accum += scrub.drag_delta().x;
                let steps = (self.width_scrub_accum / 3.0).trunc() as i64;
                if steps != 0 {
                    self.width_scrub_accum -= steps as f32 * 3.0;
                    let bits = (self.width.bits() as i64 + steps).clamp(1, 128) as u32;
                    self.set_width(bits);
                }
            }
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Sign").weak());
            if ui
                .selectable_label(self.sign == Signedness::Unsigned, "unsigned")
                .clicked()
            {
                self.set_sign(Signedness::Unsigned);
            }
            if ui
                .selectable_label(self.sign == Signedness::Signed, "signed")
                .clicked()
            {
                self.set_sign(Signedness::Signed);
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        self.fixed_point(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        self.bit_range(ui);
    }

    /// Compact current value: a base selector, the selected base shown large
    /// and editable, and the other three bases as small click-to-copy lines.
    fn current_value_compact(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "CURRENT VALUE");

        ui.horizontal(|ui| {
            for field in BASE_FIELDS {
                if ui
                    .selectable_label(self.focus_base == field, field_label(field))
                    .clicked()
                {
                    self.focus_base = field;
                }
            }
        });
        ui.add_space(4.0);

        // The selected base, large and editable — still drives every other view.
        let field = self.focus_base;
        let buf = match field {
            Field::Hex => &mut self.hex,
            Field::Dec => &mut self.dec,
            Field::Bin => &mut self.bin,
            Field::Oct => &mut self.oct,
            Field::Fixed => unreachable!(),
        };
        let resp = ui.add(
            egui::TextEdit::singleline(buf)
                .font(egui::FontId::new(20.0, egui::FontFamily::Monospace))
                .desired_width(f32::INFINITY)
                .margin(egui::vec2(8.0, 6.0)),
        );
        if resp.changed() {
            self.on_field_edit(field);
        }
        ui.add_space(6.0);

        // The other three bases, small and read-only (click to copy).
        let mut copied: Option<&'static str> = None;
        for other in BASE_FIELDS {
            if other == field {
                continue;
            }
            let label = field_label(other);
            let text = match other {
                Field::Hex => self.hex.clone(),
                Field::Dec => self.dec.clone(),
                Field::Bin => self.bin.clone(),
                Field::Oct => self.oct.clone(),
                Field::Fixed => unreachable!(),
            };
            if mini_value_line(ui, label, text.clone()) {
                ui.ctx().copy_text(text);
                copied = Some(label);
            }
        }
        if let Some(label) = copied {
            self.status = Some(format!("Copied {label}"));
            self.status_until = ui.ctx().input(|i| i.time) + 1.4;
        }
    }

    fn fixed_point(&mut self, ui: &mut egui::Ui) {
        let wbits = self.width.bits();
        let accent = theme::accent(ui.ctx());
        let int_bits = wbits.saturating_sub(self.frac_bits);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Fixed-point").weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.monospace(
                    egui::RichText::new(format!("Q{int_bits}.{}", self.frac_bits)).color(accent),
                );
            });
        });
        ui.add_space(6.0);

        // A fill bar: click or drag to set the fractional/integer split. The
        // accent fill grows from the left; the "{frac}/{width}" label floats
        // centred.
        let avail = ui.available_width();
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(avail, 22.0), egui::Sense::click_and_drag());
        if resp.clicked() || resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                let ratio = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                let frac = (ratio * wbits as f32).round() as u32;
                if frac != self.frac_bits {
                    self.frac_bits = frac.min(wbits);
                    self.refresh(None);
                }
            }
        }
        let painter = ui.painter();
        let track = ui.visuals().widgets.inactive.bg_fill;
        let radius = egui::CornerRadius::same(11);
        painter.rect_filled(rect, radius, track);
        let ratio = if wbits == 0 {
            0.0
        } else {
            self.frac_bits as f32 / wbits as f32
        };
        if ratio > 0.0 {
            let fill = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * ratio, rect.height()),
            );
            let fill_color =
                egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 184);
            painter.rect_filled(fill, radius, fill_color);
        }
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{}/{}", self.frac_bits, wbits),
            egui::FontId::monospace(12.0),
            ui.visuals().text_color(),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("real").weak());
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.fixed_input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            if resp.changed() {
                self.on_fixed_edit();
            }
        });
    }

    /// The bit-range extractor: pick `bits[hi:lo]` and read the slice back in
    /// hex/dec/bin, click-to-copy.
    fn bit_range(&mut self, ui: &mut egui::Ui) {
        let wbits = self.width.bits();
        let accent = theme::accent(ui.ctx());
        let top = wbits.saturating_sub(1);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Bit range").weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.monospace(
                    egui::RichText::new(format!("bits[{}:{}]", self.range_hi, self.range_lo))
                        .color(accent),
                );
            });
        });
        ui.add_space(6.0);

        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("hi").weak());
            ui.add(egui::DragValue::new(&mut self.range_hi).range(0..=top));
            ui.label(egui::RichText::new("lo").weak());
            ui.add(egui::DragValue::new(&mut self.range_lo).range(0..=top));
            // Normalize: clamp to width and keep hi >= lo.
            self.range_hi = self.range_hi.min(top);
            self.range_lo = self.range_lo.min(top);
            let (hi, lo) = if self.range_lo > self.range_hi {
                (self.range_lo, self.range_hi)
            } else {
                (self.range_hi, self.range_lo)
            };
            let range_width = hi - lo + 1;
            ui.label(egui::RichText::new(format!("({range_width} bits)")).weak());
        });
        ui.add_space(6.0);

        let (hi, lo) = if self.range_lo > self.range_hi {
            (self.range_lo, self.range_hi)
        } else {
            (self.range_hi, self.range_lo)
        };
        let range_width = hi - lo + 1;
        let mask = if range_width >= 128 {
            u128::MAX
        } else {
            (1u128 << range_width) - 1
        };
        let field = (self.value.raw() >> lo) & mask;
        let extract_hex = format!("0x{field:X}");
        let extract_dec = field.to_string();
        let extract_bin = format!("0b{field:0width$b}", width = range_width as usize);

        let item_fill = ui.visuals().extreme_bg_color;
        let resp = egui::Frame::group(ui.style())
            .fill(item_fill)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .corner_radius(egui::CornerRadius::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.monospace(egui::RichText::new(&extract_hex).strong());
                    ui.monospace(egui::RichText::new(&extract_dec).weak());
                    ui.monospace(egui::RichText::new(&extract_bin).weak());
                });
            })
            .response
            .interact(egui::Sense::click())
            .on_hover_text("Click to copy hex");
        if resp.clicked() {
            self.copy(ui.ctx(), extract_hex, "field");
        }
    }

    fn bits_section(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "BITS · MSB to LSB");
        let accent = theme::accent(ui.ctx());
        if let Some(new_value) = widgets::bit_grid(ui, self.value, accent) {
            self.value = new_value;
            self.status = None;
            self.refresh(None);
        }
    }

    /// Draw the auto-dismissing toast and clear it once expired.
    fn toast(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.status.is_some() && now > self.status_until {
            self.status = None;
        }
        let Some(msg) = self.status.clone() else {
            return;
        };
        egui::Area::new(egui::Id::new("powercalc_toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -22.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(16, 9))
                    .show(ui, |ui| {
                        ui.monospace(msg);
                    });
            });
        // Schedule a repaint so transient toasts dismiss without further input.
        if self.status_until.is_finite() {
            let remaining = (self.status_until - now).max(0.0);
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(remaining));
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::apply(ui.ctx(), self.theme_mode);

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Header: title + subtitle on the left, theme toggle on the right.
                ui.horizontal(|ui| {
                    ui.heading("PowerCalc");
                    ui.label(
                        egui::RichText::new("FPGA bit calculator")
                            .monospace()
                            .weak()
                            .small(),
                    );
                    // Debug-only window-size readout, so layout bugs can be
                    // reported by their exact triggering size.
                    if cfg!(debug_assertions) {
                        let sz = ui.ctx().content_rect().size();
                        ui.label(
                            egui::RichText::new(format!("{:.0}×{:.0}", sz.x, sz.y))
                                .monospace()
                                .weak()
                                .small(),
                        )
                        .on_hover_text("Window size (points). Debug builds only.");
                    }
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui
                                .button(format!("Theme · {}", self.theme_mode.label()))
                                .on_hover_text("Toggle theme: Auto, Light, Dark")
                                .clicked()
                            {
                                self.theme_mode = self.theme_mode.next();
                            }
                        },
                    );
                });
                ui.add_space(10.0);

                // The expression spans the full width on top.
                Self::section(ui, |ui| self.expression_centerpiece(ui));

                // Below it: two columns when there's room, collapsing to a
                // single stack when the window is narrow. Stacked order matches
                // column order: Current value → Bits → Format → History.
                let two_col = ui.available_width() >= 720.0;
                // `PC_DEBUG=1` dumps the layout decision (and the bit grid dumps
                // its row geometry) to stderr — cheap introspection for layout
                // bugs without needing screenshots.
                if std::env::var("PC_DEBUG").is_ok() {
                    let sz = ui.ctx().content_rect().size();
                    eprintln!(
                        "[layout] window={:.0}x{:.0} ppp={:.2} avail_w={:.1} two_col={two_col} col_w~={:.1}",
                        sz.x,
                        sz.y,
                        ui.ctx().pixels_per_point(),
                        ui.available_width(),
                        (ui.available_width() - ui.spacing().item_spacing.x) / 2.0,
                    );
                }
                if two_col {
                    ui.columns(2, |cols| {
                        Self::section(&mut cols[0], |ui| self.current_value_compact(ui));
                        Self::section(&mut cols[0], |ui| self.bits_section(ui));

                        Self::section(&mut cols[1], |ui| self.format_section(ui));
                        Self::section(&mut cols[1], |ui| self.history_panel(ui));
                    });
                } else {
                    Self::section(ui, |ui| self.current_value_compact(ui));
                    Self::section(ui, |ui| self.bits_section(ui));
                    Self::section(ui, |ui| self.format_section(ui));
                    Self::section(ui, |ui| self.history_panel(ui));
                }
            });
        });

        self.toast(ui.ctx());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("theme_mode", self.theme_mode.key().to_owned());
        storage.set_string("history_base", self.history_base.key().to_owned());
    }
}

/// The four base fields, in display order.
const BASE_FIELDS: [Field; 4] = [Field::Hex, Field::Dec, Field::Bin, Field::Oct];

fn field_label(field: Field) -> &'static str {
    match field {
        Field::Hex => "HEX",
        Field::Dec => "DEC",
        Field::Bin => "BIN",
        Field::Oct => "OCT",
        Field::Fixed => "FIX",
    }
}

/// Render a small "weak" section heading.
fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).weak().small());
    ui.add_space(4.0);
}

/// A small, weak, click-to-copy base line for the compact current-value view.
/// Returns `true` if the line was clicked (the caller copies and toasts).
fn mini_value_line(ui: &mut egui::Ui, label: &str, text: String) -> bool {
    let resp = ui
        .add(
            egui::Label::new(
                egui::RichText::new(format!("{label}  {text}"))
                    .monospace()
                    .small()
                    .weak(),
            )
            .truncate()
            .sense(egui::Sense::click()),
        )
        .on_hover_text("Click to copy");
    resp.clicked()
}

/// Render the result `value` in one base or all four, as click-to-copy lines.
/// Returns the label of a line that was clicked (for the toast), if any.
fn value_lines(
    ui: &mut egui::Ui,
    value: Value,
    sign: Signedness,
    base: HistoryBase,
) -> Option<&'static str> {
    let mut copied = None;
    let mut line = |ui: &mut egui::Ui, label: &'static str, text: String| {
        if value_line(ui, label, text) {
            copied = Some(label);
        }
    };
    match base {
        HistoryBase::All => {
            line(ui, "HEX", value.to_hex());
            line(ui, "DEC", value.to_dec(sign));
            line(ui, "BIN", value.to_bin());
            line(ui, "OCT", value.to_oct());
        }
        HistoryBase::Hex => line(ui, "HEX", value.to_hex()),
        HistoryBase::Dec => line(ui, "DEC", value.to_dec(sign)),
        HistoryBase::Bin => line(ui, "BIN", value.to_bin()),
        HistoryBase::Oct => line(ui, "OCT", value.to_oct()),
    }
    copied
}

/// One labelled, monospace, click-to-copy value line. Copies on click and
/// returns whether it was clicked (so the caller can show a toast).
fn value_line(ui: &mut egui::Ui, label: &str, text: String) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            [34.0, ui.spacing().interact_size.y],
            egui::Label::new(egui::RichText::new(label).weak().monospace().small()),
        );
        let resp = ui
            .add(
                egui::Label::new(egui::RichText::new(&text).monospace())
                    .truncate()
                    .sense(egui::Sense::click()),
            )
            .on_hover_text("Click to copy");
        if resp.clicked() {
            ui.ctx().copy_text(text.clone());
            clicked = true;
        }
    });
    clicked
}

/// Parse a base-field string into a width-masked [`Value`]. Whitespace and `_`
/// separators are ignored; hex/bin/oct accept an optional `0x`/`0b`/`0o` prefix.
/// Decimal accepts a leading `-` when in signed mode.
fn parse_base(text: &str, radix: u32, width: Width, sign: Signedness) -> Result<Value, String> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect();
    if cleaned.is_empty() {
        return Ok(Value::new(0, width));
    }

    if radix == 10 {
        if let Some(mag) = cleaned.strip_prefix('-') {
            if sign == Signedness::Unsigned {
                return Err("negative value in unsigned mode".to_owned());
            }
            let n: i128 = mag
                .parse()
                .map_err(|_| "invalid decimal number".to_owned())?;
            return Ok(Value::new((-n) as u128, width));
        }
        let n: u128 = cleaned
            .parse()
            .map_err(|_| "invalid decimal number".to_owned())?;
        return Ok(Value::new(n, width));
    }

    let body = strip_radix_prefix(&cleaned, radix);
    let n = u128::from_str_radix(body, radix)
        .map_err(|_| format!("invalid base-{radix} number"))?;
    Ok(Value::new(n, width))
}

fn strip_radix_prefix(s: &str, radix: u32) -> &str {
    let prefix = match radix {
        16 => "0x",
        2 => "0b",
        8 => "0o",
        _ => return s,
    };
    if s.len() >= 2 && s[..2].eq_ignore_ascii_case(prefix) {
        &s[2..]
    } else {
        s
    }
}
