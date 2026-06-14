//! PowerCalc application state and UI.
//!
//! One canonical [`Value`] drives everything. Editing any base field reparses
//! it and refreshes the *other* fields; toggling a bit, evaluating an
//! expression, or changing width/signedness updates the value and refreshes all
//! fields. Signedness only changes the decimal rendering and the meaning of `>>`
//! and `/`.

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

    status: Option<String>,
    theme_mode: ThemeMode,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme_mode = cc
            .storage
            .and_then(|s| s.get_string("theme_mode"))
            .and_then(|s| ThemeMode::from_key(&s))
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
        if self.expr.trim().is_empty() {
            return;
        }
        match eval(&self.expr, self.width, self.sign, self.value) {
            Ok(v) => {
                self.value = v;
                self.status = None;
                self.refresh(None);
            }
            Err(e) => self.status = Some(e.to_string()),
        }
    }

    // --- UI sections -----------------------------------------------------

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

    fn expression_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Expr:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.expr)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(360.0)
                    .hint_text("e.g. 0xFF & (1 << 3)"),
            );
            let entered =
                resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("=").clicked() || entered {
                self.eval_expr();
            }
        });

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

                Self::section(ui, |ui| self.controls_bar(ui));
                Self::section(ui, |ui| {
                    self.base_fields(ui);
                    ui.add_space(6.0);
                    self.fixed_point(ui);
                });
                Self::section(ui, |ui| {
                    ui.label(egui::RichText::new("BITS  (MSB → LSB)").weak().small());
                    ui.add_space(4.0);
                    self.bit_grid(ui);
                });
                Self::section(ui, |ui| self.expression_bar(ui));

                if let Some(msg) = &self.status {
                    ui.colored_label(egui::Color32::from_rgb(229, 115, 115), msg);
                }
            });
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("theme_mode", self.theme_mode.key().to_owned());
    }
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
