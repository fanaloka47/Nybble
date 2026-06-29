//! Panel rendering methods for [`App`].

use super::{App, Field, HistoryBase, HistoryResult, NumberMode, SettingsTab};
use crate::settings::{CopyOptions, Panel};
use crate::{theme, widgets};
use nybble_core::{f64_to_value, Signedness, Value};

impl App {
    pub(super) fn section(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
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
    pub(super) fn expression_centerpiece(&mut self, ui: &mut egui::Ui) {
        let accent = theme::accent(ui.ctx());
        let on_accent = theme::on_accent(ui.ctx());

        // Animate indicator toward target; keep repainting until settled.
        let target = if self.is_float_mode() {
            1.0_f32
        } else {
            0.0_f32
        };
        let dt = ui.input(|i| i.unstable_dt);
        self.mode_toggle_anim += (target - self.mode_toggle_anim) * (14.0 * dt).min(1.0);
        if (self.mode_toggle_anim - target).abs() > 0.001 {
            ui.ctx().request_repaint();
        }

        // Section header row: "EXPRESSION" label left, pill toggle right.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("EXPRESSION").weak().small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let h = 24.0_f32;
                let w = 110.0_f32;
                let r = (h / 2.0) as u8;
                let rr = r as f32;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
                if resp.clicked() {
                    let next = if self.is_float_mode() {
                        NumberMode::Integer
                    } else {
                        NumberMode::Float
                    };
                    self.set_number_mode(next);
                }
                let painter = ui.painter();
                let t = self.mode_toggle_anim;
                let half = w / 2.0;
                // Track
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::same(r),
                    ui.visuals().widgets.inactive.bg_fill,
                );
                // Indicator slides; outer corners round as it reaches each edge.
                let ind_x = rect.left() + t * half;
                let ind_rect =
                    egui::Rect::from_min_size(egui::pos2(ind_x, rect.top()), egui::vec2(half, h));
                let left_r = (rr * (1.0 - 2.0 * t).max(0.0)).round() as u8;
                let right_r = (rr * (2.0 * t - 1.0).max(0.0)).round() as u8;
                let ind_corners = egui::CornerRadius {
                    nw: left_r,
                    sw: left_r,
                    ne: right_r,
                    se: right_r,
                };
                painter.rect_filled(ind_rect, ind_corners, accent);
                // Labels
                let font = egui::FontId::proportional(13.0);
                let muted = ui.visuals().weak_text_color();
                painter.text(
                    egui::pos2(rect.left() + half / 2.0, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    "int",
                    font.clone(),
                    if t < 0.5 { on_accent } else { muted },
                );
                painter.text(
                    egui::pos2(rect.left() + half * 1.5, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    "float",
                    font,
                    if t > 0.5 { on_accent } else { muted },
                );
            });
        });
        ui.add_space(4.0);
        ui.add_space(6.0);

        // Hint and tooltip advertise the named functions, tailored to the mode:
        // float mode has the full scientific set, integer mode a width-masked
        // subset. The numeric logic itself lives entirely in `nybble-core`.
        let (hint, fn_help) = if self.is_float_mode() {
            (
                "sqrt(2) · sin(pi/2) · 2**8",
                "Float-mode functions:\n\
                 trig (rad): sin cos tan asin acos atan · atan2(y,x)\n\
                 trig (deg): sind cosd tand asind acosd atand\n\
                 hyperbolic: sinh cosh tanh asinh acosh atanh\n\
                 logs/exp: ln log2 log10 log(x,base) exp exp2\n\
                 powers: sqrt cbrt pow(x,y) root(x,n) · x**y\n\
                 rounding: floor ceil round trunc abs sign\n\
                 other: hypot min max gcd lcm mod fact\n\
                 constants: pi e tau",
            )
        } else {
            (
                "0xFF & (1 << 3) · log2(4)",
                "Integer-mode functions (results masked to width):\n\
                 powers: x**y · pow(x,y) · sqrt · fact\n\
                 log: log2 clog2 popcount\n\
                 sign: abs sign\n\
                 pairs: gcd lcm min max mod",
            )
        };

        ui.horizontal(|ui| {
            let mut out = egui::TextEdit::multiline(&mut self.expr)
                .font(egui::FontId::new(22.0, egui::FontFamily::Monospace))
                .desired_width(ui.available_width() - 60.0)
                .desired_rows(1)
                .hint_text(
                    egui::RichText::new(hint)
                        .font(egui::FontId::new(16.0, egui::FontFamily::Monospace)),
                )
                .margin(egui::vec2(10.0, 8.0))
                .show(ui);
            // A "send to expression" button replaced the text last frame and
            // asked us to focus; place the caret at the end so the user can type
            // an operator immediately.
            if std::mem::take(&mut self.expr_focus_request) {
                let id = out.response.response.id;
                out.response.response.request_focus();
                let end = egui::text::CCursor::new(self.expr.chars().count());
                out.state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(end)));
                out.state.store(ui.ctx(), id);
            }
            let resp = out.response.response.on_hover_text(fn_help);
            // The box is multiline so long expressions wrap instead of scrolling
            // off-screen, but it still behaves like a single field: Enter submits
            // rather than inserting a newline, so strip any newline back out.
            let entered = self.expr.contains('\n') || self.expr.contains('\r');
            if entered {
                self.expr.retain(|c| c != '\n' && c != '\r');
            }
            // Editing the expression clears any stale "invalid" message — we
            // only validate at evaluate time, never while typing.
            if resp.changed() {
                self.expr_error = None;
            }
            let clicked = ui
                .add_sized(
                    [44.0, 40.0],
                    egui::Button::new(
                        // `↵` lives only in the bundled monospace (Hack) font, so
                        // render it with the monospace family or it won't be found.
                        egui::RichText::new("↵")
                            .size(24.0)
                            .monospace()
                            .color(on_accent),
                    )
                    .fill(accent),
                )
                .on_hover_text("Evaluate (Enter)")
                .clicked();
            if clicked || entered {
                if self.eval_expr() {
                    self.flash_until = ui.input(|i| i.time) + 0.8;
                }
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
                egui::RichText::new(
                    "No expressions evaluated yet — type one above and press Enter.",
                )
                .weak(),
            );
            return;
        }

        let base = self.history_base;
        let copy = self.settings.copy;
        let accent = theme::accent(ui.ctx());
        let item_fill = ui.visuals().faint_bg_color;
        let mut recall_idx: Option<usize> = None;
        let mut copied: Option<&'static str> = None;

        // Size the list to about five entries, then scroll for the rest. An
        // entry's height depends on how many value lines the current base filter
        // shows (four for "All", one otherwise), so derive it from the row
        // metrics rather than hard-coding a pixel height.
        let row_h = ui.spacing().interact_size.y;
        let gap = ui.spacing().item_spacing.y;
        let value_rows = if base == HistoryBase::All { 4.0 } else { 1.0 };
        let entry_h = 16.0 // frame top + bottom inner margins
            + row_h // expression line
            + gap
            + value_rows * row_h
            + (value_rows - 1.0) * gap
            + 6.0 // add_space after each entry
            + gap; // spacing between entries
                   // Snug to the entries until we hit five, then cap and scroll. We reserve
                   // the box via allocate_ui so the inner scroll area gets this height — a
                   // bare nested ScrollArea would otherwise be clamped to whatever little
                   // vertical space is left in the column and collapse to ~one entry.
        let visible = (self.history.len().min(5)) as f32;
        let box_h = entry_h * visible;

        ui.allocate_ui(egui::vec2(ui.available_width(), box_h), |ui| {
            egui::ScrollArea::vertical()
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
                                let line_copied = match entry.result {
                                    HistoryResult::Integer { value, sign } => {
                                        value_lines(ui, value, sign, base, copy)
                                    }
                                    HistoryResult::Float(x) => float_value_lines(ui, x, base, copy),
                                };
                                if let Some(label) = line_copied {
                                    copied = Some(label);
                                }
                            });
                        ui.add_space(6.0);
                    }
                });
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

    /// FORMAT card: width (preset chips + drag-scrubber) and signedness.
    fn format_section(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "FORMAT");
        let accent = theme::accent(ui.ctx());

        if self.is_float_mode() {
            ui.label(
                egui::RichText::new("Full-precision f64. Width and sign apply to integer mode.")
                    .weak(),
            );
            return;
        }

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
    }

    /// INTERPRET card: fixed-point view and bit slicer.
    fn interpret_section(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "INTERPRET");

        if self.is_float_mode() {
            ui.label(
                egui::RichText::new("Fixed-point and bit slicer apply to integer mode.").weak(),
            );
            return;
        }

        let show_fixed = self.settings.show_fixed_point;
        let show_slicer = self.settings.show_bit_slicer;
        if show_fixed {
            self.fixed_point(ui);
        }
        if show_fixed && show_slicer {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
        }
        if show_slicer {
            self.bit_range(ui);
        }
    }

    /// Whether a base field is enabled in the current-value panel.
    fn field_enabled(&self, field: Field) -> bool {
        match field {
            Field::Hex => self.settings.show_hex,
            Field::Dec => self.settings.show_dec,
            Field::Bin => self.settings.show_bin,
            Field::Oct => self.settings.show_oct,
            Field::Fixed => true,
        }
    }

    /// Current value: all four bases shown stacked, each independently editable.
    fn current_value_compact(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        if self.value_just_changed {
            self.flash_until = now + 0.8;
            self.value_just_changed = false;
        }
        let flash_t = ((self.flash_until - now) / 0.8).clamp(0.0, 1.0) as f32;

        ui.horizontal(|ui| {
            section_label(ui, "CURRENT VALUE");
            if flash_t > 0.0 {
                let accent = theme::accent(ui.ctx());
                let alpha = (flash_t * 255.0) as u8;
                let color = egui::Color32::from_rgba_unmultiplied(
                    accent.r(),
                    accent.g(),
                    accent.b(),
                    alpha,
                );
                ui.label(egui::RichText::new("✓").small().monospace().color(color));
                ui.ctx().request_repaint();
            }
        });

        for field in BASE_FIELDS {
            if !self.field_enabled(field) {
                continue;
            }
            let label = field_label(field);
            let (edit_changed, enter_pressed, copy_clicked, send_clicked, buf_text) = {
                let buf = self.buffer_mut(field);
                ui.horizontal_top(|ui| {
                    ui.add_sized(
                        [36.0, ui.spacing().interact_size.y],
                        egui::Label::new(egui::RichText::new(label).weak().monospace().small()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let copy_clicked = widgets::copy_icon_button(ui).clicked();
                        let send_clicked = widgets::send_icon_button(ui).clicked();
                        let resp = ui.add(
                            egui::TextEdit::multiline(buf)
                                .font(egui::FontId::new(16.0, egui::FontFamily::Monospace))
                                .desired_width(f32::INFINITY)
                                .desired_rows(1)
                                .margin(egui::vec2(8.0, 4.0)),
                        );
                        if flash_t > 0.0 {
                            let accent = theme::accent(ui.ctx());
                            let alpha = (flash_t * 200.0) as u8;
                            let color = egui::Color32::from_rgba_unmultiplied(
                                accent.r(),
                                accent.g(),
                                accent.b(),
                                alpha,
                            );
                            ui.painter().rect_stroke(
                                resp.rect,
                                egui::CornerRadius::same(4),
                                egui::Stroke::new(1.5, color),
                                egui::StrokeKind::Outside,
                            );
                        }
                        let had_newline = buf.contains('\n') || buf.contains('\r');
                        (
                            resp.changed(),
                            had_newline,
                            copy_clicked,
                            send_clicked,
                            buf.clone(),
                        )
                    })
                    .inner
                })
                .inner
            };
            if edit_changed {
                // Strip newlines the multiline widget may insert when Enter is pressed.
                self.buffer_mut(field).retain(|c| c != '\n' && c != '\r');
                self.on_field_edit(field);
                if enter_pressed {
                    self.flash_until = ui.input(|i| i.time) + 0.8;
                    self.value_just_changed = false; // already flashing, don't double-trigger
                }
            }
            if copy_clicked {
                let text = self.settings.copy.apply(label, &buf_text);
                self.copy(ui.ctx(), text, label);
            }
            if send_clicked {
                self.expr = self.field_literal(field);
                self.expr_error = None;
                self.expr_focus_request = true;
                // The expression box was already drawn this frame; repaint so the
                // deferred focus lands on the very next frame.
                ui.ctx().request_repaint();
            }
            ui.add_space(4.0);
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
            ui.add(
                egui::TextEdit::singleline(&mut self.fixed_input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .interactive(false),
            );
        });
    }

    /// Bit slicer: pick `bits[hi:lo]` and read the slice back in
    /// hex/dec/bin, click-to-copy.
    fn bit_range(&mut self, ui: &mut egui::Ui) {
        let wbits = self.width.bits();
        let accent = theme::accent(ui.ctx());
        let top = wbits.saturating_sub(1);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Bit slicer").weak());
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
            self.invalidate_expr();
            self.refresh(None);
        }
    }

    /// Render one panel's contents (without the surrounding card).
    pub(super) fn render_panel(&mut self, ui: &mut egui::Ui, panel: Panel) {
        match panel {
            Panel::Value => self.current_value_compact(ui),
            Panel::Bits => self.bits_section(ui),
            Panel::Format => self.format_section(ui),
            Panel::Interpret => self.interpret_section(ui),
            Panel::History => self.history_panel(ui),
        }
    }

    /// Whether a panel currently has anything to show. A panel disabled in
    /// settings is hidden; so is one whose every field/sub-block is toggled off
    /// (so we never render an empty card), and Bits is hidden in float mode
    /// (the value is an f64, not width-bound bits).
    fn panel_visible(&self, panel: Panel) -> bool {
        if !self.settings.is_panel_enabled(panel) {
            return false;
        }
        match panel {
            Panel::Bits => !self.is_float_mode(),
            Panel::Value => {
                self.settings.show_hex
                    || self.settings.show_dec
                    || self.settings.show_bin
                    || self.settings.show_oct
            }
            // In integer mode the Interpret card auto-hides when both sub-blocks
            // are off; in float mode it always shows its placeholder text.
            Panel::Interpret => {
                self.is_float_mode()
                    || self.settings.show_fixed_point
                    || self.settings.show_bit_slicer
            }
            _ => true,
        }
    }

    /// The enabled, visible panels in user-chosen order.
    pub(super) fn visible_panels(&self) -> Vec<Panel> {
        self.settings
            .panel_order
            .iter()
            .copied()
            .filter(|&p| self.panel_visible(p))
            .collect()
    }

    /// Split the visible panels into two columns, assigning each in order to the
    /// column with the smaller accumulated weight so the two stay balanced and
    /// rebalance automatically as panels are toggled.
    pub(super) fn balance_columns(panels: &[Panel]) -> (Vec<Panel>, Vec<Panel>) {
        let (mut left, mut right) = (Vec::new(), Vec::new());
        let (mut lw, mut rw) = (0.0f32, 0.0f32);
        for &p in panels {
            if lw <= rw {
                left.push(p);
                lw += p.weight();
            } else {
                right.push(p);
                rw += p.weight();
            }
        }
        (left, right)
    }

    /// The field/sub-block toggles belonging to `panel`, drawn indented under
    /// its row. Panels without fields (Bits, Format, History) render nothing.
    fn panel_field_toggles(&mut self, ui: &mut egui::Ui, panel: Panel, enabled: bool) {
        if !matches!(panel, Panel::Value | Panel::Interpret) {
            return;
        }
        ui.indent(panel.key(), |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                ui.horizontal_wrapped(|ui| match panel {
                    Panel::Value => {
                        ui.checkbox(&mut self.settings.show_hex, "HEX");
                        ui.checkbox(&mut self.settings.show_dec, "DEC");
                        ui.checkbox(&mut self.settings.show_bin, "BIN");
                        ui.checkbox(&mut self.settings.show_oct, "OCT");
                    }
                    Panel::Interpret => {
                        ui.checkbox(&mut self.settings.show_fixed_point, "Fixed-point");
                        ui.checkbox(&mut self.settings.show_bit_slicer, "Bit slicer");
                    }
                    _ => {}
                });
            });
        });
    }

    /// The settings modal: a left-hand category nav and a right content pane,
    /// like a typical app's preferences window.
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        // egui's Window auto-expands to fit content but never auto-shrinks: the
        // inner Resize keeps `max(desired, last_content)` across frames (and even
        // while hidden). Because our content size tracks the window, growing the
        // OS window once would leave the modal stuck oversized and overflowing a
        // later-smaller window — and a `max_width` cap alone does not override the
        // remembered size. Pin the width (min == max) so it is forced to a
        // deterministic value each frame, and cap the height; both track the live
        // window. The content scrolls when taller than the cap.
        let avail = ctx.content_rect();
        // Text labels when the window is wide enough; icons (with tooltips) when
        // narrow, so the content pane keeps room in Compact mode.
        let text_nav = avail.width() >= 560.0;
        let nav_w = if text_nav { 104.0 } else { 40.0 };
        // Fill ~90% of the window width so there's always a margin to the edges,
        // capped so it doesn't get unwieldy on a very wide window.
        let win_w = (avail.width() * 0.9).clamp(300.0, 620.0);
        let max_h = (avail.height() - 32.0).clamp(240.0, 600.0);
        let content_w = (win_w - nav_w - 30.0).max(180.0);
        let scroll_h = (max_h - 60.0).max(140.0);

        // Custom title bar: the default one centers the title and is tall, so we
        // disable it and draw our own left-aligned, compact ribbon.
        egui::Window::new("Settings")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .min_width(win_w)
            .max_width(win_w)
            .max_height(max_h)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Settings").size(13.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::close_icon_button(ui).clicked() {
                            self.settings_open = false;
                        }
                    });
                });
                ui.separator();

                ui.horizontal_top(|ui| {
                    // Left: category navigation — text labels when wide, drawn
                    // icons (names in tooltips) when narrow to spare the content.
                    let nav_layout = if text_nav {
                        egui::Layout::top_down_justified(egui::Align::LEFT)
                    } else {
                        egui::Layout::top_down(egui::Align::Center)
                    };
                    ui.allocate_ui_with_layout(egui::vec2(nav_w, 0.0), nav_layout, |ui| {
                        if !text_nav {
                            ui.spacing_mut().item_spacing.y = 6.0;
                        }
                        for tab in SettingsTab::ALL {
                            let selected = self.settings_tab == tab;
                            let clicked = if text_nav {
                                ui.selectable_label(selected, tab.label()).clicked()
                            } else {
                                nav_icon_button(ui, tab, selected).clicked()
                            };
                            if clicked {
                                self.settings_tab = tab;
                            }
                        }
                    });
                    ui.separator();
                    // Right: the selected category's content. Bounded height so a
                    // tall pane scrolls instead of growing the window past the cap.
                    ui.vertical(|ui| {
                        ui.set_width(content_w);
                        egui::ScrollArea::vertical()
                            .max_height(scroll_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| match self.settings_tab {
                                SettingsTab::Panels => self.panels_settings(ui),
                                SettingsTab::Copy => self.copy_settings(ui),
                                SettingsTab::Expressions => expression_reference(ui),
                            });
                    });
                });
            });
    }

    /// "What's new" dialog: the running version's release notes, shown once
    /// after an update or on demand via the header version label.
    pub(super) fn changelog_window(&mut self, ctx: &egui::Context) {
        if !self.changelog_open {
            return;
        }
        let version = env!("CARGO_PKG_VERSION");
        let notes =
            crate::changelog::notes_for(version).or_else(|| crate::changelog::ENTRIES.first());

        // A Modal (auto-sizing, dimmed backdrop) rather than a Window: its height
        // tracks the content. Window's inner Resize keeps max(desired, last_content)
        // and never shrinks, which leaves a tall gap under short notes.
        let avail = ctx.content_rect();
        let win_w = (avail.width() * 0.9).clamp(280.0, 460.0);
        // Cap the notes list so a long changelog scrolls instead of growing the
        // modal past the window's bottom edge.
        let list_max_h = (avail.height() * 0.6).max(120.0);

        let modal = egui::Modal::new(egui::Id::new("whats_new")).show(ctx, |ui| {
            ui.set_width(win_w);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("What's new in v{version}"))
                        .size(13.0)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::close_icon_button(ui).clicked() {
                        self.changelog_open = false;
                    }
                });
            });
            ui.separator();

            let accent = theme::accent(ui.ctx());
            match notes {
                Some(n) => {
                    egui::ScrollArea::vertical()
                        .max_height(list_max_h)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for item in n.items {
                                ui.horizontal_top(|ui| {
                                    ui.label(egui::RichText::new("•").color(accent));
                                    // Wrap long notes within the remaining width
                                    // instead of running off the modal's edge.
                                    ui.add(egui::Label::new(*item).wrap());
                                });
                                ui.add_space(2.0);
                            }
                        });
                }
                None => {
                    ui.label(egui::RichText::new("No release notes.").weak());
                }
            }

            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Got it").clicked() {
                    self.changelog_open = false;
                }
            });
        });

        // Click outside the modal (on the dimmed backdrop) dismisses it.
        if modal.should_close() {
            self.changelog_open = false;
        }
    }

    /// Panels pane: per-panel enable + reorder, with each panel's own field
    /// toggles nested beneath it.
    fn panels_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Panels & fields").strong());
        ui.add_space(6.0);
        // Defer moves until after the loop so we never mutate the order while
        // iterating its indices.
        let mut move_up: Option<usize> = None;
        let mut move_down: Option<usize> = None;
        let order = self.settings.panel_order.clone();
        let last = order.len().saturating_sub(1);
        for (i, panel) in order.iter().copied().enumerate() {
            ui.horizontal(|ui| {
                let mut on = self.settings.is_panel_enabled(panel);
                if ui.checkbox(&mut on, panel.label()).changed() {
                    self.settings.set_panel_enabled(panel, on);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::triangle_button(ui, false, i < last)
                        .on_hover_text("Move down")
                        .clicked()
                    {
                        move_down = Some(i);
                    }
                    if widgets::triangle_button(ui, true, i > 0)
                        .on_hover_text("Move up")
                        .clicked()
                    {
                        move_up = Some(i);
                    }
                });
            });
            // A panel's fields are configured under it, greyed out when the
            // panel itself is off.
            let enabled = self.settings.is_panel_enabled(panel);
            self.panel_field_toggles(ui, panel, enabled);
        }
        if let Some(i) = move_up {
            self.settings.move_up(i);
        }
        if let Some(i) = move_down {
            self.settings.move_down(i);
        }
    }

    /// Copy pane: clipboard options with a live preview of a sample hex value.
    fn copy_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Copy behaviour").strong());
        ui.add_space(6.0);
        ui.checkbox(
            &mut self.settings.copy.prepend_prefix,
            "Prepend base prefix",
        );
        ui.checkbox(
            &mut self.settings.copy.keep_leading_zeros,
            "Keep leading zeros",
        );
        ui.checkbox(
            &mut self.settings.copy.keep_separators,
            "Keep group separators",
        );
        let preview = self.settings.copy.apply("HEX", "00DE_AD00");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Preview").weak().small());
            ui.monospace(egui::RichText::new(preview).color(theme::accent(ui.ctx())));
        });
    }

    /// Draw the auto-dismissing toast and clear it once expired.
    pub(super) fn toast(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.status.is_some() && now > self.status_until {
            self.status = None;
        }
        let Some(msg) = self.status.clone() else {
            return;
        };
        egui::Area::new(egui::Id::new("nybble_toast"))
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

/// A settings-category button: a drawn icon (the category name lives in a
/// tooltip) so the nav column stays narrow. Highlights when selected.
fn nav_icon_button(ui: &mut egui::Ui, tab: SettingsTab, selected: bool) -> egui::Response {
    let size = 30.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let accent = theme::accent(ui.ctx());
        let cr = egui::CornerRadius::same(7);
        if selected {
            ui.painter().rect_filled(rect, cr, accent);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(rect, cr, ui.visuals().widgets.hovered.bg_fill);
        }
        let col = if selected {
            theme::on_accent(ui.ctx())
        } else {
            ui.visuals().widgets.inactive.fg_stroke.color
        };
        let c = rect.center();
        match tab {
            SettingsTab::Panels => draw_panels_glyph(ui.painter(), c, col),
            SettingsTab::Copy => {
                let bg = if selected {
                    accent
                } else {
                    theme::card_fill(ui.ctx())
                };
                draw_copy_glyph(ui.painter(), c, col, bg);
            }
            SettingsTab::Expressions => draw_expr_glyph(ui.painter(), c, col),
        }
    }
    resp.on_hover_text(tab.label())
}

/// Stacked panels: three short filled bars.
fn draw_panels_glyph(p: &egui::Painter, c: egui::Pos2, col: egui::Color32) {
    let w = 14.0;
    let h = 3.0;
    let gap = 5.0;
    for i in -1..=1 {
        let r =
            egui::Rect::from_center_size(egui::pos2(c.x, c.y + i as f32 * gap), egui::vec2(w, h));
        p.rect_filled(r, egui::CornerRadius::same(1), col);
    }
}

/// Two overlapping pages (matches `copy_icon_button`'s look).
fn draw_copy_glyph(p: &egui::Painter, c: egui::Pos2, col: egui::Color32, bg: egui::Color32) {
    let stroke = egui::Stroke::new(1.3, col);
    let corner = egui::CornerRadius::same(2);
    let (pw, ph) = (9.0, 11.0);
    let back = egui::Rect::from_center_size(egui::pos2(c.x + 2.5, c.y - 2.5), egui::vec2(pw, ph));
    p.rect_stroke(back, corner, stroke, egui::StrokeKind::Middle);
    let front = egui::Rect::from_center_size(egui::pos2(c.x - 2.5, c.y + 2.5), egui::vec2(pw, ph));
    p.rect_filled(front, corner, bg);
    p.rect_stroke(front, corner, stroke, egui::StrokeKind::Middle);
}

/// A sine curve, signalling math / expressions.
fn draw_expr_glyph(p: &egui::Painter, c: egui::Pos2, col: egui::Color32) {
    let stroke = egui::Stroke::new(1.7, col);
    let w = 18.0;
    let amp = 4.5;
    let n = 24;
    let pts: Vec<egui::Pos2> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let x = c.x - w / 2.0 + t * w;
            let y = c.y - (t * std::f32::consts::TAU).sin() * amp;
            egui::pos2(x, y)
        })
        .collect();
    p.add(egui::Shape::line(pts, stroke));
}

/// Render a small "weak" section heading.
fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).weak().small());
    ui.add_space(4.0);
}

/// A two-column reference table: a fixed-width monospace token on the left and a
/// wrapping prose description on the right (wrapping keeps it inside a narrow
/// window instead of forcing the modal wider).
fn ref_table(ui: &mut egui::Ui, rows: &[(&str, &str)], token_w: f32) {
    for (token, desc) in rows {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.allocate_ui_with_layout(
                egui::vec2(token_w, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.monospace(egui::RichText::new(*token).strong());
                },
            );
            ui.add(egui::Label::new(egui::RichText::new(*desc).weak()).wrap());
        });
    }
}

/// The expression-language reference, rendered natively. Curated from
/// `docs/expressions.md` (kept in step with the `nybble-core` evaluators); not a
/// verbatim copy. ASCII-only so it renders in the bundled JetBrains Mono.
fn expression_reference(ui: &mut egui::Ui) {
    let accent = theme::accent(ui.ctx());
    let heading = |ui: &mut egui::Ui, text: &str| {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(text).strong().color(accent));
        ui.add_space(2.0);
    };
    let note = |ui: &mut egui::Ui, text: &str| {
        ui.add_space(2.0);
        ui.label(egui::RichText::new(text).weak().small());
    };

    ui.label(egui::RichText::new("Expressions").strong());
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(
            "Type an expression and press Enter; the result becomes the current \
             value. Integer and float mode share one grammar — switch with the \
             int/float toggle. Integer results wrap and mask to the active bit \
             width; float is full f64.",
        )
        .weak(),
    );

    heading(ui, "Literals");
    ref_table(
        ui,
        &[
            ("42", "decimal"),
            ("0xFF", "hexadecimal"),
            ("0b1010", "binary"),
            ("0o17", "octal"),
            ("1.5", "decimal fraction (float only)"),
            ("1e6", "scientific (float only)"),
        ],
        110.0,
    );
    note(ui, "_ may group digits anywhere: 1_000, 0xDEAD_BEEF.");

    heading(ui, "Names & constants");
    ref_table(
        ui,
        &[
            ("ans", "the current value (both modes)"),
            ("pi", "3.14159... (float only)"),
            ("e", "2.71828... (float only)"),
            ("tau", "2*pi = 6.28318... (float only)"),
        ],
        110.0,
    );

    heading(ui, "Operators - integer mode");
    ref_table(
        ui,
        &[
            ("| ^ &", "bitwise or, xor, and"),
            ("~", "bitwise not"),
            ("<< >>", "shift left / right (arithmetic when signed)"),
            ("+ - * / %", "arithmetic; / and % are sign-aware"),
            ("**", "power, right-associative"),
            ("-x", "two's-complement negate"),
        ],
        110.0,
    );
    note(ui, "Every result is re-masked to the active width.");

    heading(ui, "Operators - float mode");
    ref_table(
        ui,
        &[
            ("+ - * / %", "arithmetic; 1/0 = inf, 0/0 = nan"),
            ("**", "power, right-associative; 2 ** 0.5 = 1.4142"),
            ("-x", "negate"),
        ],
        110.0,
    );
    note(
        ui,
        "Bitwise and shift operators are rejected in float mode.",
    );

    heading(ui, "Functions - integer mode");
    ref_table(
        ui,
        &[
            ("pow(x, y)", "x to the power y (= x ** y)"),
            ("sqrt(x)", "integer square root (floor)"),
            ("log2(x)", "floor of log2 (error if x = 0)"),
            ("clog2(x)", "ceil of log2; bits to index x values"),
            ("popcount(x)", "number of set bits"),
            ("abs(x)", "absolute value (sign-aware)"),
            ("sign(x)", "-1, 0, or 1 (sign-aware)"),
            ("fact(x)", "factorial (wraps to width)"),
            ("gcd(x, y)", "greatest common divisor"),
            ("lcm(x, y)", "least common multiple"),
            ("min(x, y)", "smaller value (sign-aware)"),
            ("max(x, y)", "larger value (sign-aware)"),
            ("mod(x, y)", "remainder (= %)"),
        ],
        110.0,
    );

    heading(ui, "Functions - float mode");
    let group = |ui: &mut egui::Ui, label: &str, fns: &str| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(egui::RichText::new(format!("{label}:")).weak());
            // Force wrapping: a bare monospace label in a horizontal layout would
            // extend to its full width and push the modal wider than the window.
            ui.add(egui::Label::new(egui::RichText::new(fns).monospace()).wrap());
        });
    };
    group(
        ui,
        "trig (rad)",
        "sin cos tan · asin acos atan · atan2(y,x)",
    );
    group(ui, "trig (deg)", "sind cosd tand · asind acosd atand");
    group(ui, "hyperbolic", "sinh cosh tanh · asinh acosh atanh");
    group(ui, "logs / exp", "ln log10 log2 · log(x,base) · exp exp2");
    group(ui, "powers / roots", "sqrt cbrt · pow(x,y) · root(x,n)");
    group(ui, "rounding", "floor ceil round trunc · abs sign");
    group(ui, "helpers", "hypot min max mod gcd lcm fact");
    note(
        ui,
        "Out-of-domain calls (e.g. sqrt(-1)) return NaN, never an error.",
    );

    heading(ui, "Examples");
    ref_table(
        ui,
        &[
            ("0xFF & (1 << 3)", "= 8"),
            ("clog2(1024)", "= 10"),
            ("2 ** 8", "= 256"),
            ("gcd(54, 24)", "= 6"),
            ("sqrt(2)", "= 1.41421..."),
            ("sin(pi / 2)", "= 1"),
            ("log(8, 2)", "= 3"),
            ("hypot(3, 4)", "= 5"),
        ],
        126.0,
    );
}

/// Render the result `value` in one base or all four, as click-to-copy lines.
/// Returns the label of a line that was clicked (for the toast), if any.
fn value_lines(
    ui: &mut egui::Ui,
    value: Value,
    sign: Signedness,
    base: HistoryBase,
    copy: CopyOptions,
) -> Option<&'static str> {
    let mut copied = None;
    let mut line = |ui: &mut egui::Ui, label: &'static str, text: String| {
        if value_line(ui, label, text, copy) {
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

/// Render a float result: the full-precision decimal plus, for the bit bases,
/// its f64 IEEE-754 pattern. Mirrors [`value_lines`] for float history entries.
fn float_value_lines(
    ui: &mut egui::Ui,
    x: f64,
    base: HistoryBase,
    copy: CopyOptions,
) -> Option<&'static str> {
    let bits = f64_to_value(x);
    let mut copied = None;
    let mut line = |ui: &mut egui::Ui, label: &'static str, text: String| {
        if value_line(ui, label, text, copy) {
            copied = Some(label);
        }
    };
    match base {
        HistoryBase::All => {
            line(ui, "DEC", format!("{x}"));
            line(ui, "HEX", bits.to_hex());
            line(ui, "BIN", bits.to_bin());
            line(ui, "OCT", bits.to_oct());
        }
        HistoryBase::Dec => line(ui, "DEC", format!("{x}")),
        HistoryBase::Hex => line(ui, "HEX", bits.to_hex()),
        HistoryBase::Bin => line(ui, "BIN", bits.to_bin()),
        HistoryBase::Oct => line(ui, "OCT", bits.to_oct()),
    }
    copied
}

/// One labelled, monospace, click-to-copy value line. Copies on click and
/// returns whether it was clicked (so the caller can show a toast).
fn value_line(ui: &mut egui::Ui, label: &str, text: String, copy: CopyOptions) -> bool {
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
            ui.ctx().copy_text(copy.apply(label, &text));
            clicked = true;
        }
    });
    clicked
}
