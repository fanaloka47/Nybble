//! Nybble application state and UI.
//!
//! The expression field is the centerpiece: type an expression, evaluate it, and
//! it becomes the current value *and* an entry in the history. One canonical
//! [`Value`] drives everything — the live base fields, the bit grid, and the
//! history all read from it. Signedness only changes the decimal rendering and
//! the meaning of `>>` and `/`.

use nybble_core::{eval, eval_float, f64_to_value, fixed, Signedness, Value, Width};

use crate::settings::{CopyOptions, Panel, Settings};
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
    /// Tolerance (logical points) for treating a window size as "on preset".
    const SNAP_TOLERANCE: f32 = 4.0;

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

/// Categories in the settings modal's left-hand navigation. Session-only state.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SettingsTab {
    #[default]
    Panels,
    Copy,
    Expressions,
}

impl SettingsTab {
    const ALL: [SettingsTab; 3] = [
        SettingsTab::Panels,
        SettingsTab::Copy,
        SettingsTab::Expressions,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsTab::Panels => "Panels",
            SettingsTab::Copy => "Copy",
            SettingsTab::Expressions => "Expressions",
        }
    }
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
    /// Set when a "send to expression" button is clicked; consumed next frame
    /// by `expression_centerpiece` to focus the box (which is drawn before the
    /// value rows, so the focus request must be deferred one frame).
    expr_focus_request: bool,

    /// Transient bottom-of-screen toast (copies, parse errors).
    status: Option<String>,
    /// `input.time` after which a transient toast auto-dismisses. Parse errors
    /// use [`f64::INFINITY`] so they persist until the next successful action.
    status_until: f64,
    /// `input.time` until which the "✓ updated" flash on the current-value
    /// section is visible. Set on every successful value update.
    flash_until: f64,
    /// Set to `true` by any direct value mutation; consumed at the top of
    /// `current_value_compact` to start the flash (avoids needing `ui` in
    /// the mutation methods, which don't have access to the current time).
    value_just_changed: bool,

    /// `bits[hi:lo]` extraction range.
    range_hi: u32,
    range_lo: u32,
    /// Sub-pixel accumulator for the width drag-scrubber (3px per bit).
    width_scrub_accum: f32,
    /// Animation progress for the int/float toggle (0.0 = int, 1.0 = float).
    mode_toggle_anim: f32,

    theme_mode: ThemeMode,
    view_mode: ViewMode,
    /// View/copy configuration edited via the settings modal.
    settings: Settings,
    /// Whether the settings modal window is open.
    settings_open: bool,
    /// Selected category in the settings modal.
    settings_tab: SettingsTab,
    /// Last window size set by a manual drag; `None` until the first resize.
    custom_size: Option<egui::Vec2>,
    /// Frames remaining before the resize-detection check re-arms after a
    /// programmatic resize (avoids a false "manual resize" on the next frame).
    resize_cooldown: u8,
    /// True on the very first frame: sends the startup resize command once the
    /// event loop is running (calling send_viewport_cmd in new() resets the
    /// Wayland connection before the loop is ready).
    startup_resize_pending: bool,
    /// Last observed `pixels_per_point`. A change means the window moved to a
    /// monitor with a different scale factor; used to suppress the resize
    /// detector across DPI transitions.
    last_ppp: f32,

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
        let custom_size = storage.and_then(|s| {
            let w = s.get_string("custom_w")?.parse::<f32>().ok()?;
            let h = s.get_string("custom_h")?.parse::<f32>().ok()?;
            Some(egui::vec2(w, h))
        });
        let auto_check_updates = storage
            .and_then(|s| s.get_string("auto_check_updates"))
            .map(|v| v != "false")
            .unwrap_or(true);
        let settings = storage.map(Settings::load).unwrap_or_default();

        // Register JetBrains Mono as the primary monospace font to match the design.
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "JetBrainsMono".to_owned(),
            egui::FontData::from_static(include_bytes!(
                "../../../resources/fonts/JetBrainsMono-Regular.ttf"
            ))
            .into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "JetBrainsMono".to_owned());
        cc.egui_ctx.set_fonts(fonts);

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
            expr_focus_request: false,
            status: None,
            status_until: 0.0,
            flash_until: 0.0,
            value_just_changed: false,
            range_hi: 7,
            range_lo: 0,
            width_scrub_accum: 0.0,
            mode_toggle_anim: if number_mode == NumberMode::Float {
                1.0
            } else {
                0.0
            },
            theme_mode,
            view_mode,
            settings,
            settings_open: false,
            settings_tab: SettingsTab::default(),
            custom_size,
            resize_cooldown: 0,
            startup_resize_pending: true,
            last_ppp: 0.0,
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

    /// Clear the expression field when the value is mutated outside of eval.
    fn invalidate_expr(&mut self) {
        self.expr.clear();
        self.expr_error = None;
        self.value_just_changed = true;
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
        self.invalidate_expr();
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
    /// The current value of `field`, formatted as an expression-ready literal
    /// (with the base prefix). Underscore group separators are accepted by the
    /// expression tokenizer, so the buffers can be fed back verbatim.
    fn field_literal(&self, field: Field) -> String {
        match field {
            Field::Hex => format!("0x{}", self.hex),
            Field::Bin => format!("0b{}", self.bin),
            Field::Oct => format!("0o{}", self.oct),
            Field::Dec => self.dec.clone(),
            Field::Fixed => unreachable!("fixed field is not a base field"),
        }
    }

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
                self.invalidate_expr();
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
                        self.invalidate_expr();
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
                        self.invalidate_expr();
                        self.refresh(Some(field));
                    }
                    Err(e) => self.error(e),
                }
            }
            Field::Fixed => unreachable!("fixed field hidden in float mode"),
        }
    }

    fn eval_expr(&mut self) -> bool {
        let trimmed = self.expr.trim().to_owned();
        if trimmed.is_empty() {
            return false;
        }
        if self.is_float_mode() {
            match eval_float(&self.expr, self.float_value) {
                Ok(x) => {
                    self.float_value = x;
                    self.expr_error = None;
                    self.status = None;
                    self.push_history(trimmed, HistoryResult::Float(x));
                    self.refresh(None);
                    return true;
                }
                Err(e) => self.expr_error = Some(format!("Invalid expression: {e}")),
            }
            return false;
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
                true
            }
            Err(e) => {
                self.expr_error = Some(format!("Invalid expression: {e}"));
                false
            }
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
                        egui::Label::new(egui::RichText::new(label).weak().monospace().small()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let copy_clicked = copy_icon_button(ui).clicked();
                        let send_clicked = send_icon_button(ui).clicked();
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
                let buf: &mut String = match field {
                    Field::Hex => &mut self.hex,
                    Field::Dec => &mut self.dec,
                    Field::Bin => &mut self.bin,
                    Field::Oct => &mut self.oct,
                    Field::Fixed => unreachable!(),
                };
                buf.retain(|c| c != '\n' && c != '\r');
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
    fn render_panel(&mut self, ui: &mut egui::Ui, panel: Panel) {
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
    fn visible_panels(&self) -> Vec<Panel> {
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
    fn balance_columns(panels: &[Panel]) -> (Vec<Panel>, Vec<Panel>) {
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
    fn settings_window(&mut self, ctx: &egui::Context) {
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
        let nav_w = 104.0;
        let win_w = (avail.width() - 24.0).clamp(300.0, 620.0);
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
                        if close_icon_button(ui).clicked() {
                            self.settings_open = false;
                        }
                    });
                });
                ui.separator();

                ui.horizontal_top(|ui| {
                    // Left: category navigation. A left-justified layout makes
                    // each item fill the nav width with its label aligned left.
                    ui.allocate_ui_with_layout(
                        egui::vec2(nav_w, 0.0),
                        egui::Layout::top_down_justified(egui::Align::LEFT),
                        |ui| {
                            for tab in SettingsTab::ALL {
                                let selected = self.settings_tab == tab;
                                if ui.selectable_label(selected, tab.label()).clicked() {
                                    self.settings_tab = tab;
                                }
                            }
                        },
                    );
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
                    if triangle_button(ui, false, i < last)
                        .on_hover_text("Move down")
                        .clicked()
                    {
                        move_down = Some(i);
                    }
                    if triangle_button(ui, true, i > 0)
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

/// Clamp a desired logical window size to roughly fit the current monitor.
///
/// Presets are fixed logical sizes; on a small or high-DPI screen (e.g. a 4K
/// panel at 150%+) the `Full` preset can exceed the monitor's usable area, so
/// Windows would clamp/reposition the window and the preset would never "take".
/// We shrink the request to fit, leaving a margin for the title bar and
/// taskbar, but never below the app's minimum size. If the monitor size isn't
/// known yet (e.g. the very first frame), the size is returned unchanged.
fn clamp_to_monitor(ctx: &egui::Context, size: egui::Vec2) -> egui::Vec2 {
    const MIN: egui::Vec2 = egui::vec2(520.0, 480.0);
    // Rough allowance for window chrome and the taskbar (logical points).
    const MARGIN: egui::Vec2 = egui::vec2(32.0, 96.0);
    match ctx.input(|i| i.viewport().monitor_size) {
        Some(monitor) if monitor.x > 0.0 && monitor.y > 0.0 => {
            let avail = (monitor - MARGIN).max(MIN);
            size.min(avail)
        }
        _ => size,
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
            let startup_size = clamp_to_monitor(ui.ctx(), startup_size);
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(startup_size));
            self.resize_cooldown = 20;
        }

        // Per-monitor DPI scale. On a Windows multi-monitor setup this changes
        // when the window crosses to a screen with a different scale factor
        // (e.g. an HD panel at 100% to a 4K panel at 150%). content_rect() is
        // reported in logical points, so a ppp change makes the size appear to
        // jump even though the physical window is unchanged — which the resize
        // detector below would misread as a manual resize and flip to Custom.
        // Arm the cooldown for a few frames so the DPI transition settles.
        let ppp = ui.ctx().pixels_per_point();
        if (ppp - self.last_ppp).abs() > f32::EPSILON {
            self.last_ppp = ppp;
            self.resize_cooldown = self.resize_cooldown.max(3);
        }

        // Detect manual window resize using content_rect, which is always
        // available (unlike inner_rect which is often None under WSL/glow).
        // Skip during the cooldown that follows every programmatic resize or
        // DPI change. Preset sizes are compared after clamping to the current
        // monitor, so a preset that had to shrink to fit a smaller/high-DPI
        // screen still counts as "on preset" rather than demoting to Custom.
        let current_size = ui.ctx().content_rect().size();
        if self.resize_cooldown > 0 {
            self.resize_cooldown -= 1;
        } else {
            let compact = clamp_to_monitor(ui.ctx(), ViewMode::COMPACT_SIZE);
            let full = clamp_to_monitor(ui.ctx(), ViewMode::FULL_SIZE);
            match self.view_mode {
                ViewMode::Compact | ViewMode::Full => {
                    let expected = if self.view_mode == ViewMode::Compact {
                        compact
                    } else {
                        full
                    };
                    if (current_size - expected).length() > ViewMode::SNAP_TOLERANCE {
                        // Snap to a preset if the size happens to match one;
                        // otherwise enter Custom and remember this size.
                        if (current_size - compact).length() <= ViewMode::SNAP_TOLERANCE {
                            self.view_mode = ViewMode::Compact;
                        } else if (current_size - full).length() <= ViewMode::SNAP_TOLERANCE {
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
                    ui.label(
                        egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
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
                            if let Some(mode) = theme_icon_toggle(ui, self.theme_mode) {
                                self.theme_mode = mode;
                            }
                            if settings_icon_button(ui).clicked() {
                                self.settings_open = !self.settings_open;
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
                                let new_size = clamp_to_monitor(ui.ctx(), new_size);
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
                // single stack when the window is narrow. Panels render in the
                // user-configured order; the two-column split keeps that order
                // while balancing the columns by rough height.
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
                let visible = self.visible_panels();
                if two_col {
                    let (left, right) = Self::balance_columns(&visible);
                    ui.columns(2, |cols| {
                        for &p in &left {
                            Self::section(&mut cols[0], |ui| self.render_panel(ui, p));
                        }
                        for &p in &right {
                            Self::section(&mut cols[1], |ui| self.render_panel(ui, p));
                        }
                    });
                } else {
                    for p in visible {
                        Self::section(ui, |ui| self.render_panel(ui, p));
                    }
                }
            });
        });

        self.settings_window(ui.ctx());
        self.toast(ui.ctx());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("theme_mode", self.theme_mode.key().to_owned());
        storage.set_string("history_base", self.history_base.key().to_owned());
        storage.set_string("view_mode", self.view_mode.key().to_owned());
        storage.set_string("number_mode", self.number_mode.key().to_owned());
        storage.set_string(
            "auto_check_updates",
            if self.auto_check_updates {
                "true"
            } else {
                "false"
            }
            .to_owned(),
        );
        if let Some(sz) = self.custom_size {
            storage.set_string("custom_w", sz.x.to_string());
            storage.set_string("custom_h", sz.y.to_string());
        }
        self.settings.save(storage);
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
fn send_icon_button(ui: &mut egui::Ui) -> egui::Response {
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
fn settings_icon_button(ui: &mut egui::Ui) -> egui::Response {
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
fn close_icon_button(ui: &mut egui::Ui) -> egui::Response {
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
    resp.on_hover_text("Close")
}

/// Small up/down reorder button drawn as a filled triangle (JetBrains Mono has
/// no arrow glyphs, so we paint it). When `enabled` is false it renders greyed
/// and ignores clicks.
fn triangle_button(ui: &mut egui::Ui, up: bool, enabled: bool) -> egui::Response {
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
/// Returns the next ThemeMode if clicked.
fn theme_icon_toggle(ui: &mut egui::Ui, current: ThemeMode) -> Option<ThemeMode> {
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

/// Crescent moon: filled circle with a same-background-colour "bite" circle.
fn draw_moon(p: &egui::Painter, c: egui::Pos2, col: egui::Color32, bg: egui::Color32) {
    p.circle_filled(c, 5.5, col);
    p.circle_filled(egui::pos2(c.x + 2.5, c.y - 2.5), 4.2, bg);
}

/// Half-filled circle: left half solid, right half just an outline.
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

/// Sun: small filled circle with six short rays.
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
    let n =
        u128::from_str_radix(body, radix).map_err(|_| format!("invalid base-{radix} number"))?;
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
