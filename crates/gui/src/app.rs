//! Nybble application state and UI.
//!
//! The expression field is the centerpiece: type an expression, evaluate it, and
//! it becomes the current value *and* an entry in the history. One canonical
//! [`Value`] drives everything — the live base fields, the bit grid, and the
//! history all read from it. Signedness only changes the decimal rendering and
//! the meaning of `>>` and `/`.

use nybble_core::{eval, eval_float, f64_to_value, fixed, Signedness, Value, Width};

use crate::theme::{self, ThemeMode};
use crate::widgets;

enum UpdateMsg {
    Available(String),
    UpToDate,
    Failed,
    Applied,
}

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

/// How the calculator interprets expressions and the current value.
///
/// `Integer` is the default programmer's-calculator behaviour (width-bound
/// two's-complement bits). `Float` evaluates in full-precision `f64` for the
/// occasional non-integer calculation. The active mode is resolved through the
/// single [`App::is_float_mode`] accessor, so changing *how* float mode is
/// triggered later (e.g. implicit promotion) is a one-place edit.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum NumberMode {
    #[default]
    Integer,
    Float,
}

impl NumberMode {
    fn key(self) -> &'static str {
        match self {
            NumberMode::Integer => "integer",
            NumberMode::Float => "float",
        }
    }

    fn from_key(s: &str) -> Option<NumberMode> {
        match s {
            "integer" => Some(NumberMode::Integer),
            "float" => Some(NumberMode::Float),
            _ => None,
        }
    }
}

/// Logical window-sizing mode.
///
/// - `Compact` forces a single narrow column (420 × 700 px).
/// - `Full` uses the wide two-column layout (880 × 760 px).
/// - `Custom` tracks the last manually-resized window size; entered
///   automatically whenever the user drags the window border.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum ViewMode {
    Compact,
    #[default]
    Full,
    Custom,
}

impl ViewMode {
    const COMPACT_SIZE: egui::Vec2 = egui::vec2(420.0, 700.0);
    const FULL_SIZE: egui::Vec2 = egui::vec2(880.0, 760.0);

    fn label(self) -> &'static str {
        match self {
            ViewMode::Compact => "Compact",
            ViewMode::Full => "Full",
            ViewMode::Custom => "Custom",
        }
    }

    fn key(self) -> &'static str {
        match self {
            ViewMode::Compact => "compact",
            ViewMode::Full => "full",
            ViewMode::Custom => "custom",
        }
    }

    fn from_key(s: &str) -> Option<ViewMode> {
        match s {
            "compact" => Some(ViewMode::Compact),
            "full" => Some(ViewMode::Full),
            "custom" => Some(ViewMode::Custom),
            _ => None,
        }
    }
}

/// Which base(s) the history list shows for each result.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum HistoryBase {
    All,
    Hex,
    #[default]
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

/// The result of an evaluated expression, in the mode that produced it.
/// Integer results carry the sign in effect so the decimal stays faithful;
/// float results carry the full-precision `f64`.
#[derive(Clone, Copy)]
enum HistoryResult {
    Integer { value: Value, sign: Signedness },
    Float(f64),
}

/// One evaluated expression and the result it produced.
#[derive(Clone)]
struct HistoryEntry {
    expr: String,
    result: HistoryResult,
}

pub struct App {
    value: Value,
    width: Width,
    sign: Signedness,
    frac_bits: u32,

    /// Integer vs. full-precision float evaluation. Read everywhere via
    /// [`App::is_float_mode`]; the mode toggle is its only writer.
    number_mode: NumberMode,
    /// The current value in float mode, and the `ans` source there. Kept
    /// separate from `value` so switching modes never corrupts the other.
    float_value: f64,

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
    view_mode: ViewMode,
    /// Last window size set by a manual drag; `None` until the first resize.
    custom_size: Option<egui::Vec2>,
    /// Frames remaining before the resize-detection check re-arms after a
    /// programmatic resize (avoids a false "manual resize" on the next frame).
    resize_cooldown: u8,
    /// True on the very first frame: sends the startup resize command once the
    /// event loop is running (calling send_viewport_cmd in new() resets the
    /// Wayland connection before the loop is ready).
    startup_resize_pending: bool,

    // Auto-update
    auto_check_updates: bool,
    update_rx: Option<std::sync::mpsc::Receiver<UpdateMsg>>,
    update_available: Option<String>,
    updating: bool,
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
        let view_mode = storage
            .and_then(|s| s.get_string("view_mode"))
            .and_then(|s| ViewMode::from_key(&s))
            .unwrap_or_default();
        let number_mode = storage
            .and_then(|s| s.get_string("number_mode"))
            .and_then(|s| NumberMode::from_key(&s))
            .unwrap_or_default();
        let custom_size = storage
            .and_then(|s| {
                let w = s.get_string("custom_w")?.parse::<f32>().ok()?;
                let h = s.get_string("custom_h")?.parse::<f32>().ok()?;
                Some(egui::vec2(w, h))
            });
        let auto_check_updates = storage
            .and_then(|s| s.get_string("auto_check_updates"))
            .map(|v| v != "false")
            .unwrap_or(true);

        let width = Width::new(32).unwrap();
        let mut app = Self {
            value: Value::new(0, width),
            width,
            sign: Signedness::Unsigned,
            frac_bits: 0,
            number_mode,
            float_value: 0.0,
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
            status_until: 0.0,
            range_hi: 7,
            range_lo: 0,
            width_scrub_accum: 0.0,
            theme_mode,
            view_mode,
            custom_size,
            resize_cooldown: 0,
            startup_resize_pending: true,
            auto_check_updates,
            update_rx: None,
            update_available: None,
            updating: false,
        };
        app.refresh(None);
        if auto_check_updates {
            app.spawn_update_check(cc.egui_ctx.clone());
        }
        app
    }

    fn spawn_update_check(&mut self, ctx: egui::Context) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.update_rx = Some(rx);
        std::thread::spawn(move || {
            let msg = match crate::update::newer_release() {
                Ok(Some(v)) => UpdateMsg::Available(v),
                Ok(None) => UpdateMsg::UpToDate,
                Err(_) => UpdateMsg::Failed,
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn drain_update_rx(&mut self) {
        if let Some(rx) = &self.update_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    UpdateMsg::Available(v) => self.update_available = Some(v),
                    UpdateMsg::UpToDate | UpdateMsg::Failed => {}
                    UpdateMsg::Applied => {
                        crate::update::restart();
                    }
                }
            }
        }
    }

    fn spawn_apply_update(&mut self, ctx: egui::Context) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.update_rx = Some(rx);
        self.updating = true;
        std::thread::spawn(move || {
            let msg = match crate::update::apply_update() {
                Ok(_) => UpdateMsg::Applied,
                Err(_) => UpdateMsg::Failed,
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
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
        if self.is_float_mode() {
            // Decimal shows the full-precision float; the bit rows show its
            // f64 IEEE-754 pattern (always 64-bit, independent of `width`).
            let bits = f64_to_value(self.float_value);
            if skip != Some(Field::Dec) {
                self.dec = format!("{}", self.float_value);
            }
            if skip != Some(Field::Hex) {
                self.hex = bits.to_hex();
            }
            if skip != Some(Field::Bin) {
                self.bin = bits.to_bin();
            }
            if skip != Some(Field::Oct) {
                self.oct = bits.to_oct();
            }
            return;
        }
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

    /// The single source of truth for whether float mode is active. Change this
    /// (and `set_number_mode`) to alter how float mode is triggered.
    fn is_float_mode(&self) -> bool {
        self.number_mode == NumberMode::Float
    }

    /// Switch modes, seeding the destination from the current value so a result
    /// carries over (e.g. `ans` stays meaningful across a toggle).
    fn set_number_mode(&mut self, mode: NumberMode) {
        if mode == self.number_mode {
            return;
        }
        match mode {
            NumberMode::Float => {
                self.float_value = match self.sign {
                    Signedness::Unsigned => self.value.as_unsigned() as f64,
                    Signedness::Signed => self.value.as_signed() as f64,
                };
            }
            // Integer keeps whatever `value` already holds; the float result is
            // left untouched in `float_value` for when we switch back.
            NumberMode::Integer => {}
        }
        self.number_mode = mode;
        self.expr_error = None;
        self.status = None;
        self.refresh(None);
    }

    /// Parse the buffer for `field`, update the value, and refresh the others.
    fn on_field_edit(&mut self, field: Field) {
        if self.is_float_mode() {
            self.on_field_edit_float(field);
            return;
        }
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

    /// Field editing in float mode: the decimal field accepts a real number;
    /// the hex/bin/oct fields reinterpret an entered 64-bit pattern as an f64.
    fn on_field_edit_float(&mut self, field: Field) {
        match field {
            Field::Dec => {
                let text = self.dec.trim();
                if text.is_empty() {
                    return;
                }
                match text.parse::<f64>() {
                    Ok(x) => {
                        self.float_value = x;
                        self.status = None;
                        self.refresh(Some(field));
                    }
                    Err(_) => self.error("invalid real number"),
                }
            }
            Field::Hex | Field::Bin | Field::Oct => {
                let (text, radix) = match field {
                    Field::Hex => (self.hex.clone(), 16),
                    Field::Bin => (self.bin.clone(), 2),
                    Field::Oct => (self.oct.clone(), 8),
                    _ => unreachable!(),
                };
                let w64 = Width::new(64).unwrap();
                match parse_base(&text, radix, w64, Signedness::Unsigned) {
                    Ok(v) => {
                        self.float_value = f64::from_bits(v.raw() as u64);
                        self.status = None;
                        self.refresh(Some(field));
                    }
                    Err(e) => self.error(e),
                }
            }
            Field::Fixed => unreachable!("fixed field hidden in float mode"),
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
        if self.is_float_mode() {
            match eval_float(&self.expr, self.float_value) {
                Ok(x) => {
                    self.float_value = x;
                    self.expr_error = None;
                    self.status = None;
                    self.push_history(trimmed, HistoryResult::Float(x));
                    self.refresh(None);
                }
                Err(e) => self.expr_error = Some(format!("Invalid expression: {e}")),
            }
            return;
        }
        match eval(&self.expr, self.width, self.sign, self.value) {
            Ok(v) => {
                self.value = v;
                self.expr_error = None;
                self.status = None;
                self.push_history(
                    trimmed,
                    HistoryResult::Integer {
                        value: v,
                        sign: self.sign,
                    },
                );
                self.refresh(None);
            }
            Err(e) => self.expr_error = Some(format!("Invalid expression: {e}")),
        }
    }

    fn push_history(&mut self, expr: String, result: HistoryResult) {
        self.history.push(HistoryEntry { expr, result });
        const MAX_HISTORY: usize = 200;
        if self.history.len() > MAX_HISTORY {
            let excess = self.history.len() - MAX_HISTORY;
            self.history.drain(0..excess);
        }
    }

    /// Bring a history entry back: restore its mode, value, and text.
    fn recall(&mut self, entry: HistoryEntry) {
        match entry.result {
            HistoryResult::Integer { value, sign } => {
                self.number_mode = NumberMode::Integer;
                self.value = value;
                self.width = value.width();
                self.sign = sign;
                if self.frac_bits > self.width.bits() {
                    self.frac_bits = self.width.bits();
                }
            }
            HistoryResult::Float(x) => {
                self.number_mode = NumberMode::Float;
                self.float_value = x;
            }
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

        // The int/float mode toggle lives next to the expression it governs, so
        // switching tasks is one click away from where you type.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Mode").weak());
            if ui.selectable_label(!self.is_float_mode(), "int").clicked() {
                self.set_number_mode(NumberMode::Integer);
            }
            if ui.selectable_label(self.is_float_mode(), "float").clicked() {
                self.set_number_mode(NumberMode::Float);
            }
        });
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.expr)
                    .font(egui::FontId::new(22.0, egui::FontFamily::Monospace))
                    .desired_width(ui.available_width() - 60.0)
                    .hint_text(
                        egui::RichText::new("0xFF & (1 << 3)")
                            .font(egui::FontId::new(16.0, egui::FontFamily::Monospace)),
                    )
                    .vertical_align(egui::Align::Center)
                    .margin(egui::vec2(10.0, 8.0)),
            );
            paint_input_frame(ui, resp.rect, accent);
            // Editing the expression clears any stale "invalid" message — we
            // only validate at evaluate time, never while typing.
            if resp.changed() {
                self.expr_error = None;
            }
            let entered =
                resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let clicked = ui
                .add_sized(
                    [44.0, 40.0],
                    egui::Button::new(
                        // `↵` lives only in the bundled monospace (Hack) font, so
                        // render it with the monospace family or it won't be found.
                        egui::RichText::new("↵").size(24.0).monospace().color(on_accent),
                    )
                    .fill(accent),
                )
                .on_hover_text("Evaluate (Enter)")
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
                                        value_lines(ui, value, sign, base)
                                    }
                                    HistoryResult::Float(x) => float_value_lines(ui, x, base),
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

    /// The FORMAT card: width (preset chips + a drag-scrubber), sign, the
    /// fixed-point split, and the bit-range extractor.
    fn format_section(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "FORMAT");
        let accent = theme::accent(ui.ctx());

        // In float mode the value is a full-precision f64, so the integer-only
        // controls below (width, sign, fixed-point) don't apply.
        if self.is_float_mode() {
            ui.label(
                egui::RichText::new(
                    "Full-precision f64. Width, sign, and fixed-point apply to integer mode.",
                )
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

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        self.fixed_point(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        self.bit_range(ui);
    }

    /// Current value: all four bases shown stacked, each independently editable.
    fn current_value_compact(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "CURRENT VALUE");
        let accent = theme::accent(ui.ctx());

        for field in BASE_FIELDS {
            let label = field_label(field);
            let (edit_changed, copy_clicked, buf_text) = {
                let buf: &mut String = match field {
                    Field::Hex => &mut self.hex,
                    Field::Dec => &mut self.dec,
                    Field::Bin => &mut self.bin,
                    Field::Oct => &mut self.oct,
                    Field::Fixed => unreachable!(),
                };
                ui.horizontal_top(|ui| {
                    ui.add_sized(
                        [36.0, ui.spacing().interact_size.y],
                        egui::Label::new(
                            egui::RichText::new(label).weak().monospace().small(),
                        ),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let copy_clicked = copy_icon_button(ui).clicked();
                        let resp = ui.add(
                            egui::TextEdit::multiline(buf)
                                .font(egui::FontId::new(16.0, egui::FontFamily::Monospace))
                                .desired_width(f32::INFINITY)
                                .desired_rows(1)
                                .margin(egui::vec2(8.0, 4.0)),
                        );
                        paint_input_frame(ui, resp.rect, accent);
                        (resp.changed(), copy_clicked, buf.clone())
                    })
                    .inner
                })
                .inner
            };
            if edit_changed {
                // Strip newlines the multiline widget may insert when Enter is pressed.
                let buf: &mut String = match field {
                    Field::Hex => &mut self.hex,
                    Field::Dec => &mut self.dec,
                    Field::Bin => &mut self.bin,
                    Field::Oct => &mut self.oct,
                    Field::Fixed => unreachable!(),
                };
                buf.retain(|c| c != '\n' && c != '\r');
                self.on_field_edit(field);
            }
            if copy_clicked {
                self.copy(ui.ctx(), buf_text, label);
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
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.fixed_input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            paint_input_frame(ui, resp.rect, accent);
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_update_rx();
        theme::apply(ui.ctx(), self.theme_mode);

        // On the very first frame the event loop is running — safe to resize.
        if self.startup_resize_pending {
            self.startup_resize_pending = false;
            let startup_size = match self.view_mode {
                ViewMode::Compact => ViewMode::COMPACT_SIZE,
                ViewMode::Full => ViewMode::FULL_SIZE,
                ViewMode::Custom => self.custom_size.unwrap_or(ViewMode::FULL_SIZE),
            };
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(startup_size));
            self.resize_cooldown = 20;
        }

        // Detect manual window resize using content_rect, which is always
        // available (unlike inner_rect which is often None under WSL/glow).
        // Skip during the cooldown that follows every programmatic resize.
        let current_size = ui.ctx().content_rect().size();
        if self.resize_cooldown > 0 {
            self.resize_cooldown -= 1;
        } else {
            match self.view_mode {
                ViewMode::Compact | ViewMode::Full => {
                    let expected = if self.view_mode == ViewMode::Compact {
                        ViewMode::COMPACT_SIZE
                    } else {
                        ViewMode::FULL_SIZE
                    };
                    if (current_size - expected).length() > 4.0 {
                        // Snap to a preset if the size happens to match one;
                        // otherwise enter Custom and remember this size.
                        if (current_size - ViewMode::COMPACT_SIZE).length() <= 4.0 {
                            self.view_mode = ViewMode::Compact;
                        } else if (current_size - ViewMode::FULL_SIZE).length() <= 4.0 {
                            self.view_mode = ViewMode::Full;
                        } else {
                            self.view_mode = ViewMode::Custom;
                            self.custom_size = Some(current_size);
                        }
                    }
                }
                // While in Custom the user may keep resizing; track every
                // change so the last manually-set size is always saved.
                ViewMode::Custom => {
                    self.custom_size = Some(current_size);
                }
            }
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Header: title + subtitle on the left, theme toggle on the right.
                ui.horizontal(|ui| {
                    ui.heading("Nybble");
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
                            // Cycle: Compact → Full → Custom (if exists) → Compact
                            let view_label = self.view_mode.label();
                            if ui
                                .button(view_label)
                                .on_hover_text("Cycle view: Compact / Full / Custom")
                                .clicked()
                            {
                                let (next_mode, new_size) = match self.view_mode {
                                    ViewMode::Compact => {
                                        (ViewMode::Full, ViewMode::FULL_SIZE)
                                    }
                                    ViewMode::Full => match self.custom_size {
                                        Some(sz) => (ViewMode::Custom, sz),
                                        None => (ViewMode::Compact, ViewMode::COMPACT_SIZE),
                                    },
                                    ViewMode::Custom => {
                                        (ViewMode::Compact, ViewMode::COMPACT_SIZE)
                                    }
                                };
                                self.view_mode = next_mode;
                                self.resize_cooldown = 10;
                                ui.ctx().send_viewport_cmd(
                                    egui::ViewportCommand::InnerSize(new_size),
                                );
                            }

                            // Update banner / controls (right-to-left, so leftmost = last).
                            if let Some(ref v) = self.update_available.clone() {
                                let label = if self.updating {
                                    "Updating…".to_owned()
                                } else {
                                    format!("Update & restart (v{v})")
                                };
                                if ui
                                    .add_enabled(
                                        !self.updating,
                                        egui::Button::new(
                                            egui::RichText::new(label)
                                                .color(theme::on_accent(ui.ctx())),
                                        )
                                        .fill(theme::accent(ui.ctx())),
                                    )
                                    .on_hover_text("Download the new version and restart")
                                    .clicked()
                                {
                                    self.spawn_apply_update(ui.ctx().clone());
                                }
                            } else if !self.updating && self.update_rx.is_none() {
                                if ui
                                    .button("Check for updates")
                                    .on_hover_text("Check GitHub Releases for a newer version")
                                    .clicked()
                                {
                                    self.spawn_update_check(ui.ctx().clone());
                                }
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
                let two_col =
                    self.view_mode != ViewMode::Compact && ui.available_width() >= 720.0;
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
                // The bit grid edits the integer value, so it is hidden in
                // float mode (where the value is an f64, not width-bound bits).
                let show_bits = !self.is_float_mode();
                if two_col {
                    ui.columns(2, |cols| {
                        Self::section(&mut cols[0], |ui| self.current_value_compact(ui));
                        if show_bits {
                            Self::section(&mut cols[0], |ui| self.bits_section(ui));
                        }

                        Self::section(&mut cols[1], |ui| self.format_section(ui));
                        Self::section(&mut cols[1], |ui| self.history_panel(ui));
                    });
                } else {
                    Self::section(ui, |ui| self.current_value_compact(ui));
                    if show_bits {
                        Self::section(ui, |ui| self.bits_section(ui));
                    }
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
        storage.set_string("view_mode", self.view_mode.key().to_owned());
        storage.set_string("number_mode", self.number_mode.key().to_owned());
        storage.set_string(
            "auto_check_updates",
            if self.auto_check_updates { "true" } else { "false" }.to_owned(),
        );
        if let Some(sz) = self.custom_size {
            storage.set_string("custom_w", sz.x.to_string());
            storage.set_string("custom_h", sz.y.to_string());
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

/// Paint a 3 px accent-colored stripe on the left edge of `rect` to mark the
/// field as an editable input.
/// Decorate an editable field: a subtle full outline so its bounds are easy to
/// read, plus an accent stripe down the left edge marking it as editable.
fn paint_input_frame(ui: &mut egui::Ui, rect: egui::Rect, accent: egui::Color32) {
    let painter = ui.painter();
    // Outline the whole box so its edges are legible against the card fill.
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(4),
        egui::Stroke::new(1.0, accent.gamma_multiply(0.55)),
        egui::StrokeKind::Inside,
    );
    // Accent stripe down the left edge.
    painter.rect_filled(
        egui::Rect::from_min_max(rect.left_top(), egui::pos2(rect.left() + 3.0, rect.bottom())),
        egui::CornerRadius::same(2),
        accent,
    );
}

/// Drawn "copy" icon button: two overlapping rectangles, no font dependency.
fn copy_icon_button(ui: &mut egui::Ui) -> egui::Response {
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
        let back = egui::Rect::from_center_size(
            egui::pos2(cx + 2.5, cy - 2.0),
            egui::vec2(pw, ph),
        );
        ui.painter().rect_stroke(back, corner, stroke, egui::StrokeKind::Middle);

        // Front page (offset down-left), filled to occlude the back page.
        let front = egui::Rect::from_center_size(
            egui::pos2(cx - 1.5, cy + 2.0),
            egui::vec2(pw, ph),
        );
        ui.painter().rect_filled(front, corner, bg);
        ui.painter().rect_stroke(front, corner, stroke, egui::StrokeKind::Middle);
    }

    resp.on_hover_text("Copy")
}

/// Render a small "weak" section heading.
fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).weak().small());
    ui.add_space(4.0);
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

/// Render a float result: the full-precision decimal plus, for the bit bases,
/// its f64 IEEE-754 pattern. Mirrors [`value_lines`] for float history entries.
fn float_value_lines(ui: &mut egui::Ui, x: f64, base: HistoryBase) -> Option<&'static str> {
    let bits = f64_to_value(x);
    let mut copied = None;
    let mut line = |ui: &mut egui::Ui, label: &'static str, text: String| {
        if value_line(ui, label, text) {
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
