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
    custom_width: u32,

    // Text buffers backing the editable fields.
    hex: String,
    dec: String,
    bin: String,
    oct: String,
    fixed_input: String,
    expr: String,

    history: Vec<HistoryEntry>,
    history_base: HistoryBase,

    /// Error from the last expression evaluation, shown inline under the field.
    /// Only set at evaluate time; cleared when the expression is edited.
    expr_error: Option<String>,
    status: Option<String>,
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
            custom_width: 32,
            hex: String::new(),
            dec: String::new(),
            bin: String::new(),
            oct: String::new(),
            fixed_input: String::new(),
            expr: String::new(),
            history: Vec::new(),
            history_base,
            expr_error: None,
            status: None,
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

    fn set_width(&mut self, bits: u32) {
        self.width = Width::clamped(bits);
        self.custom_width = self.width.bits();
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
            Err(e) => self.status = Some(e),
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
            Err(_) => self.status = Some("invalid real number".to_owned()),
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
        self.custom_width = self.width.bits();
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
            ui.add_space(4.0);
            ui.colored_label(egui::Color32::from_rgb(229, 115, 115), err);
        }

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            const OPS: &[(&str, &str)] = &[
                ("AND", " & "),
                ("OR", " | "),
                ("XOR", " ^ "),
                ("NOT", "~"),
                ("SHL", " << "),
                ("SHR", " >> "),
                ("(", "("),
                (")", ")"),
                ("+", " + "),
                ("-", " - "),
                ("*", " * "),
                ("/", " / "),
                ("ans", "ans"),
            ];
            for (label, token) in OPS {
                if ui.button(*label).clicked() {
                    self.expr.push_str(token);
                }
            }
            if ui.button("Clear").clicked() {
                self.expr.clear();
            }
        });
    }

    fn history_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            section_label(ui, "HISTORY");
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
                            value_lines(ui, entry.value, entry.sign, base);
                        });
                    ui.add_space(6.0);
                }
            });

        if let Some(i) = recall_idx {
            let entry = self.history[i].clone();
            self.recall(entry);
        }
    }

    fn controls_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Width:");
            for bits in [8u32, 16, 32, 64] {
                if ui
                    .selectable_label(self.width.bits() == bits, bits.to_string())
                    .clicked()
                {
                    self.set_width(bits);
                }
            }
            ui.separator();
            ui.label("custom");
            let mut custom = self.custom_width;
            if ui.add(egui::Slider::new(&mut custom, 1..=128)).changed() {
                self.set_width(custom);
            }
        });

        ui.horizontal(|ui| {
            ui.label("Sign:");
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
    }

    fn base_fields(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("bases")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                self.base_row(ui, "HEX", Field::Hex);
                self.base_row(ui, "DEC", Field::Dec);
                self.base_row(ui, "BIN", Field::Bin);
                self.base_row(ui, "OCT", Field::Oct);
            });
    }

    fn base_row(&mut self, ui: &mut egui::Ui, label: &str, field: Field) {
        ui.label(egui::RichText::new(label).strong());
        let buf = match field {
            Field::Hex => &mut self.hex,
            Field::Dec => &mut self.dec,
            Field::Bin => &mut self.bin,
            Field::Oct => &mut self.oct,
            Field::Fixed => unreachable!(),
        };
        let resp = ui.add(
            egui::TextEdit::singleline(buf)
                .font(egui::TextStyle::Monospace)
                .desired_width(420.0),
        );
        ui.end_row();
        if resp.changed() {
            self.on_field_edit(field);
        }
    }

    fn fixed_point(&mut self, ui: &mut egui::Ui) {
        let wbits = self.width.bits();
        ui.horizontal(|ui| {
            let int_bits = wbits.saturating_sub(self.frac_bits);
            ui.label(format!("Fixed-point  Q{int_bits}.{}", self.frac_bits));
            if ui
                .add(egui::Slider::new(&mut self.frac_bits, 0..=wbits).text("frac bits"))
                .changed()
            {
                self.refresh(None);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Real:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.fixed_input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(220.0),
            );
            if resp.changed() {
                self.on_fixed_edit();
            }
        });
    }

    fn bit_grid(&mut self, ui: &mut egui::Ui) {
        let accent = theme::accent(ui.ctx());
        if let Some(new_value) = widgets::bit_grid(ui, self.value, accent) {
            self.value = new_value;
            self.status = None;
            self.refresh(None);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::apply(ui.ctx(), self.theme_mode);

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Header: title on the left, theme toggle on the right.
                ui.horizontal(|ui| {
                    ui.heading("PowerCalc");
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui
                                .button(format!("Theme: {}", self.theme_mode.label()))
                                .on_hover_text("Toggle theme: Auto → Light → Dark")
                                .clicked()
                            {
                                self.theme_mode = self.theme_mode.next();
                            }
                        },
                    );
                });
                ui.add_space(8.0);

                // The expression field is the centerpiece.
                Self::section(ui, |ui| self.expression_centerpiece(ui));

                Self::section(ui, |ui| {
                    section_label(ui, "CURRENT VALUE");
                    self.base_fields(ui);
                    ui.add_space(6.0);
                    self.fixed_point(ui);
                });

                Self::section(ui, |ui| self.history_panel(ui));

                Self::section(ui, |ui| {
                    section_label(ui, "FORMAT");
                    self.controls_bar(ui);
                });

                Self::section(ui, |ui| {
                    section_label(ui, "BITS  (MSB → LSB)");
                    self.bit_grid(ui);
                });

                if let Some(msg) = &self.status {
                    ui.colored_label(egui::Color32::from_rgb(229, 115, 115), msg);
                }
            });
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("theme_mode", self.theme_mode.key().to_owned());
        storage.set_string("history_base", self.history_base.key().to_owned());
    }
}

/// Render a small "weak" section heading.
fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).weak().small());
    ui.add_space(4.0);
}

/// Render the result `value` in one base or all four, as click-to-copy lines.
fn value_lines(ui: &mut egui::Ui, value: Value, sign: Signedness, base: HistoryBase) {
    match base {
        HistoryBase::All => {
            value_line(ui, "HEX", value.to_hex());
            value_line(ui, "DEC", value.to_dec(sign));
            value_line(ui, "BIN", value.to_bin());
            value_line(ui, "OCT", value.to_oct());
        }
        HistoryBase::Hex => value_line(ui, "HEX", value.to_hex()),
        HistoryBase::Dec => value_line(ui, "DEC", value.to_dec(sign)),
        HistoryBase::Bin => value_line(ui, "BIN", value.to_bin()),
        HistoryBase::Oct => value_line(ui, "OCT", value.to_oct()),
    }
}

/// One labelled, monospace, click-to-copy value line.
fn value_line(ui: &mut egui::Ui, label: &str, text: String) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [34.0, ui.spacing().interact_size.y],
            egui::Label::new(egui::RichText::new(label).weak().monospace().small()),
        );
        let resp = ui.add(
            egui::Label::new(egui::RichText::new(&text).monospace()).sense(egui::Sense::click()),
        );
        if resp.clicked() {
            ui.ctx().copy_text(text.clone());
        }
        resp.on_hover_text("Click to copy");
    });
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
