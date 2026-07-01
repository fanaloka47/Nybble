//! The batch list-converter workspace.
//!
//! Two vertically-scrolling columns share a single scroll area so they stay
//! locked together and line up row-for-row: the user types a list of numbers on
//! the left (one per line) and each line is converted, in place, on the right.
//! Two dropdowns pick the source and target base.
//!
//! Values are parsed to their natural magnitude (up to 128 bits) and rendered
//! with minimal digits — no fixed width, decimal always unsigned. The numeric
//! work is entirely `nybble-core`; this module is presentation only.

use nybble_core::{parse_base, Signedness, Value, Width};

use super::App;
use crate::theme;

/// One of the four numeric bases the converter reads from / writes to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Base {
    Hex,
    Dec,
    Bin,
    Oct,
}

impl Base {
    const ALL: [Base; 4] = [Base::Hex, Base::Dec, Base::Bin, Base::Oct];

    fn radix(self) -> u32 {
        match self {
            Base::Hex => 16,
            Base::Dec => 10,
            Base::Bin => 2,
            Base::Oct => 8,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Base::Hex => "HEX",
            Base::Dec => "DEC",
            Base::Bin => "BIN",
            Base::Oct => "OCT",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Base::Hex => "hex",
            Base::Dec => "dec",
            Base::Bin => "bin",
            Base::Oct => "oct",
        }
    }

    pub fn from_key(s: &str) -> Option<Base> {
        match s {
            "hex" => Some(Base::Hex),
            "dec" => Some(Base::Dec),
            "bin" => Some(Base::Bin),
            "oct" => Some(Base::Oct),
            _ => None,
        }
    }
}

/// The left-dropdown selection: a fixed source base, or auto-detection from the
/// list itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SourceBase {
    Auto,
    Fixed(Base),
}

impl SourceBase {
    const ALL: [SourceBase; 5] = [
        SourceBase::Auto,
        SourceBase::Fixed(Base::Hex),
        SourceBase::Fixed(Base::Dec),
        SourceBase::Fixed(Base::Bin),
        SourceBase::Fixed(Base::Oct),
    ];

    fn label(self) -> &'static str {
        match self {
            SourceBase::Auto => "Auto-detect",
            SourceBase::Fixed(b) => b.label(),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            SourceBase::Auto => "auto",
            SourceBase::Fixed(b) => b.key(),
        }
    }

    pub fn from_key(s: &str) -> Option<SourceBase> {
        match s {
            "auto" => Some(SourceBase::Auto),
            other => Base::from_key(other).map(SourceBase::Fixed),
        }
    }

    /// The concrete base to parse with; runs [`detect_base`] over `input` for
    /// [`SourceBase::Auto`].
    fn resolve(self, input: &str) -> Base {
        match self {
            SourceBase::Fixed(b) => b,
            SourceBase::Auto => detect_base(input),
        }
    }
}

/// Guess the source base from the first 20 non-empty lines.
///
/// Explicit base prefixes (`0x`/`0b`/`0o`) are the strongest signal and win by
/// majority. With no prefixes, the digit alphabet decides: any hex letter means
/// hex; an 8 or 9 means decimal; only 0/1 means binary; a run of `0-7` digits
/// with a leading zero (the C-style octal convention) means octal; anything
/// else falls back to decimal. Empty input defaults to decimal.
fn detect_base(input: &str) -> Base {
    let sample: Vec<&str> = input
        .split('\n')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(20)
        .collect();
    if sample.is_empty() {
        return Base::Dec;
    }

    // Explicit prefixes are the strongest signal; go with the most common one.
    let (mut hex_p, mut bin_p, mut oct_p) = (0u32, 0u32, 0u32);
    for t in &sample {
        let low = t.to_ascii_lowercase();
        if low.starts_with("0x") {
            hex_p += 1;
        } else if low.starts_with("0b") {
            bin_p += 1;
        } else if low.starts_with("0o") {
            oct_p += 1;
        }
    }
    if hex_p + bin_p + oct_p > 0 {
        return if hex_p >= oct_p && hex_p >= bin_p {
            Base::Hex
        } else if oct_p >= bin_p {
            Base::Oct
        } else {
            Base::Bin
        };
    }

    // No prefixes: infer from the digits used across the sample.
    let (mut hex_letter, mut dec_digit, mut has_2_to_7) = (false, false, false);
    let mut all_leading_zero = true;
    for t in &sample {
        let body = t.strip_prefix('-').unwrap_or(t);
        if !(body.len() > 1 && body.starts_with('0')) {
            all_leading_zero = false;
        }
        for c in body.chars() {
            match c {
                '_' | '\'' | '0' | '1' => {}
                '2'..='7' => has_2_to_7 = true,
                '8' | '9' => dec_digit = true,
                'a'..='f' | 'A'..='F' => hex_letter = true,
                _ => {}
            }
        }
    }
    if hex_letter {
        Base::Hex
    } else if dec_digit {
        Base::Dec
    } else if !has_2_to_7 {
        Base::Bin
    } else if all_leading_zero {
        Base::Oct
    } else {
        Base::Dec
    }
}

/// Convert a single bare token from `from` to `to`, rendered with minimal digits
/// (no zero-padding), grouped for readability. A blank token yields an empty
/// string so line alignment is preserved; a parse failure yields the error text.
fn convert_line(token: &str, from: Base, to: Base) -> Result<String, String> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    // Parse to the full magnitude (128-bit, unsigned) so nothing is truncated.
    let v = parse_base(
        trimmed,
        from.radix(),
        Width::new(128).unwrap(),
        Signedness::Unsigned,
    )?;
    let n = v.raw();
    // Re-fit to the fewest bits that hold the value so the rendered token isn't
    // padded out to 128 bits.
    let bits = (128 - n.leading_zeros()).max(1);
    let disp = Value::new(n, Width::clamped(bits));
    Ok(match to {
        Base::Hex => disp.to_hex(),
        Base::Bin => disp.to_bin(),
        Base::Oct => disp.to_oct(),
        Base::Dec => disp.to_dec(Signedness::Unsigned),
    })
}

/// A base-picker dropdown writing back into `current`.
fn base_combo(ui: &mut egui::Ui, id: &str, current: &mut Base) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.label())
        .show_ui(ui, |ui| {
            for b in Base::ALL {
                ui.selectable_value(current, b, b.label());
            }
        });
}

/// The source-base dropdown; includes the Auto-detect option.
fn source_combo(ui: &mut egui::Ui, id: &str, current: &mut SourceBase) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.label())
        .show_ui(ui, |ui| {
            for b in SourceBase::ALL {
                ui.selectable_value(current, b, b.label());
            }
        });
}

impl App {
    pub(super) fn batch_body(&mut self, ui: &mut egui::Ui) {
        let accent = theme::accent(ui.ctx());

        // Controls: from -> to, plus a right-aligned "Copy all". The auto-detected
        // base is shown on the status line below, not here, so it never crowds
        // the dropdowns and the button on a narrow window.
        let mut copy_all = false;
        Self::section(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("From").weak().small());
                source_combo(ui, "batch_from", &mut self.batch_from);
                ui.label(egui::RichText::new("→").monospace().weak());
                ui.label(egui::RichText::new("To").weak().small());
                base_combo(ui, "batch_to", &mut self.batch_to);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("Copy all")
                        .on_hover_text("Copy the converted list (honours Copy settings)")
                        .clicked()
                    {
                        copy_all = true;
                    }
                });
            });
        });

        let from = self.batch_from.resolve(&self.batch_input);
        let to = self.batch_to;

        // Per-line counts for the summary.
        let mut n_values = 0usize;
        let mut n_errors = 0usize;
        for line in self.batch_input.split('\n') {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            n_values += 1;
            if convert_line(t, from, to).is_err() {
                n_errors += 1;
            }
        }

        // Status line (its own row, so it never crowds the controls): the
        // auto-detected base when in Auto mode, then the value/error counts. The
        // count is always shown (0 when the list is empty).
        let show_detected = self.batch_from == SourceBase::Auto && n_values > 0;
        ui.horizontal(|ui| {
            if show_detected {
                // `from` already resolved to the detected base above.
                ui.label(
                    egui::RichText::new(format!("detected: {}", from.label()))
                        .color(accent)
                        .small(),
                )
                .on_hover_text("Source base guessed from the first 20 lines");
                ui.label(egui::RichText::new("·").weak().small());
            }
            let counts = if n_errors > 0 {
                format!("{n_values} values · {n_errors} errors")
            } else {
                format!("{n_values} values")
            };
            ui.label(egui::RichText::new(counts).weak().small());
        });
        ui.add_space(4.0);

        // Build the output column: one line per input line so the two stay
        // row-aligned (blank stays blank, a bad token shows an inline "!!"
        // marker). Kept in lockstep with the input's line count.
        let mut output = String::new();
        for (i, line) in self.batch_input.split('\n').enumerate() {
            if i > 0 {
                output.push('\n');
            }
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            match convert_line(t, from, to) {
                Ok(s) => output.push_str(&s),
                Err(e) => {
                    output.push_str("!! ");
                    output.push_str(&e);
                }
            }
        }

        // Two non-wrapping text boxes in a shared vertical scroll: each value
        // stays on a single line so the columns line up row-for-row, and each
        // column scrolls sideways on its own for long values. The input is a
        // single TextEdit, so ordinary list editing — selecting, copying or
        // deleting a range of lines — works as usual.
        //
        // egui's ScrollArea deliberately makes a multiline TextEdit wrap to the
        // viewport rather than overflow (it prefers wrapping to a horizontal
        // scrollbar), and `desired_width` can't override that. The way around it
        // is a custom `layouter` that lays the text out with an infinite wrap
        // width: the galley then keeps each line whole, overflows the viewport,
        // and the horizontal scroll area scrolls to reveal it.
        let font = egui::FontId::new(15.0, egui::FontFamily::Monospace);
        let make_layouter = |font: egui::FontId| {
            move |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
                let job = egui::text::LayoutJob::simple(
                    buf.as_str().to_owned(),
                    font.clone(),
                    ui.visuals().text_color(),
                    f32::INFINITY,
                );
                ui.fonts_mut(|f| f.layout_job(job))
            }
        };
        let mut input_layouter = make_layouter(font.clone());
        let mut output_layouter = make_layouter(font.clone());

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.columns(2, |cols| {
                    cols[0].label(egui::RichText::new("INPUT").weak().small());
                    cols[0].add_space(4.0);
                    egui::ScrollArea::horizontal()
                        .id_salt("batch_input_h")
                        .auto_shrink([false, true])
                        .show(&mut cols[0], |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.batch_input)
                                    .font(font.clone())
                                    .desired_rows(20)
                                    .desired_width(f32::INFINITY)
                                    .layouter(&mut input_layouter)
                                    .hint_text("Paste values, one per line"),
                            );
                        });

                    cols[1].label(egui::RichText::new("OUTPUT").weak().small());
                    cols[1].add_space(4.0);
                    egui::ScrollArea::horizontal()
                        .id_salt("batch_output_h")
                        .auto_shrink([false, true])
                        .show(&mut cols[1], |ui| {
                            // A `&str` buffer is a read-only `TextBuffer`, so the
                            // output is selectable and copyable but not editable.
                            let mut output_ref: &str = &output;
                            ui.add(
                                egui::TextEdit::multiline(&mut output_ref)
                                    .font(font.clone())
                                    .desired_rows(20)
                                    .desired_width(f32::INFINITY)
                                    .layouter(&mut output_layouter),
                            );
                        });
                });
            });

        if copy_all {
            let label = self.batch_to.label();
            let mut lines = Vec::new();
            for line in self.batch_input.split('\n') {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(s) = convert_line(line, from, to) {
                    lines.push(self.settings.copy.apply(label, &s));
                }
            }
            let text = lines.join("\n");
            self.copy(ui.ctx(), text, "list");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_to_dec_groups() {
        assert_eq!(
            convert_line("DEADBEEF", Base::Hex, Base::Dec).unwrap(),
            "3'735'928'559"
        );
    }

    #[test]
    fn dec_to_hex_minimal_digits() {
        assert_eq!(convert_line("255", Base::Dec, Base::Hex).unwrap(), "FF");
    }

    #[test]
    fn dec_to_bin() {
        assert_eq!(convert_line("10", Base::Dec, Base::Bin).unwrap(), "1010");
    }

    #[test]
    fn bin_to_oct() {
        assert_eq!(convert_line("1010", Base::Bin, Base::Oct).unwrap(), "12");
    }

    #[test]
    fn hex_to_hex_is_not_width_padded() {
        // A short value stays short — no zero-padding out to a fixed width.
        assert_eq!(convert_line("FF", Base::Hex, Base::Hex).unwrap(), "FF");
    }

    #[test]
    fn prefix_and_separators_accepted() {
        assert_eq!(
            convert_line("0xDEAD_BEEF", Base::Hex, Base::Dec).unwrap(),
            "3'735'928'559"
        );
    }

    #[test]
    fn empty_line_stays_empty() {
        assert_eq!(convert_line("   ", Base::Hex, Base::Dec).unwrap(), "");
    }

    #[test]
    fn invalid_digit_is_error() {
        assert!(convert_line("ZZ", Base::Hex, Base::Dec).is_err());
    }

    #[test]
    fn detect_empty_defaults_to_dec() {
        assert_eq!(detect_base(""), Base::Dec);
        assert_eq!(detect_base("\n  \n"), Base::Dec);
    }

    #[test]
    fn detect_prefixes_win() {
        assert_eq!(detect_base("0xFF\n0x10\n0xAB"), Base::Hex);
        assert_eq!(detect_base("0b1010\n0b0011"), Base::Bin);
        assert_eq!(detect_base("0o17\n0o755"), Base::Oct);
    }

    #[test]
    fn detect_hex_letters() {
        assert_eq!(detect_base("DEAD\nBEEF\n10"), Base::Hex);
    }

    #[test]
    fn detect_decimal_from_8_or_9() {
        assert_eq!(detect_base("10\n29\n300"), Base::Dec);
    }

    #[test]
    fn detect_binary_from_only_0_and_1() {
        assert_eq!(detect_base("1010\n0110\n1"), Base::Bin);
    }

    #[test]
    fn detect_octal_from_leading_zero_run() {
        assert_eq!(detect_base("0755\n0644\n0022"), Base::Oct);
    }

    #[test]
    fn detect_decimal_when_0_to_7_without_leading_zero() {
        // Ambiguous 0-7 digits with no octal signal fall back to decimal.
        assert_eq!(detect_base("17\n23\n45"), Base::Dec);
    }

    #[test]
    fn detect_uses_only_first_20_lines() {
        // 20 hex-lettered lines, then a decimal one that must not sway it.
        let mut input = "AB\n".repeat(20);
        input.push_str("999");
        assert_eq!(detect_base(&input), Base::Hex);
    }
}
