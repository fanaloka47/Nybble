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

/// One of the four numeric bases the converter reads from / writes to.
#[derive(Clone, Copy, PartialEq, Eq)]
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

impl App {
    pub(super) fn batch_body(&mut self, ui: &mut egui::Ui) {
        let from = self.batch_from;
        let to = self.batch_to;

        // Controls: from -> to, plus a right-aligned "Copy all".
        let mut copy_all = false;
        Self::section(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("From").weak().small());
                base_combo(ui, "batch_from", &mut self.batch_from);
                ui.label(egui::RichText::new("->").weak());
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

        // Build the output column and per-line counts from the current input.
        let mut output = String::new();
        let mut n_values = 0usize;
        let mut n_errors = 0usize;
        for (i, line) in self.batch_input.split('\n').enumerate() {
            if i > 0 {
                output.push('\n');
            }
            if line.trim().is_empty() {
                continue;
            }
            n_values += 1;
            match convert_line(line, from, to) {
                Ok(s) => output.push_str(&s),
                Err(e) => {
                    n_errors += 1;
                    output.push_str(&format!("!! {e}"));
                }
            }
        }

        if n_values > 0 {
            let summary = if n_errors > 0 {
                format!("{n_values} values · {n_errors} errors")
            } else {
                format!("{n_values} values")
            };
            ui.label(egui::RichText::new(summary).weak().small());
            ui.add_space(4.0);
        }

        // The two columns live in one scroll area so scrolling and row heights
        // stay in lockstep; both text areas use the same monospace metrics.
        let font = egui::FontId::new(15.0, egui::FontFamily::Monospace);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.columns(2, |cols| {
                    cols[0].label(egui::RichText::new("INPUT").weak().small());
                    cols[0].add_space(4.0);
                    cols[0].add(
                        egui::TextEdit::multiline(&mut self.batch_input)
                            .font(font.clone())
                            .desired_rows(20)
                            .desired_width(f32::INFINITY)
                            .hint_text("Paste values, one per line"),
                    );

                    cols[1].label(egui::RichText::new("OUTPUT").weak().small());
                    cols[1].add_space(4.0);
                    cols[1].add(
                        egui::TextEdit::multiline(&mut output)
                            .font(font.clone())
                            .desired_rows(20)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
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
}
