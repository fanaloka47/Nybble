//! Nybble application state and UI.
//!
//! The expression field is the centerpiece: type an expression, evaluate it, and
//! it becomes the current value *and* an entry in the history. One canonical
//! [`Value`] drives everything — the live base fields, the bit grid, and the
//! history all read from it. Signedness only changes the decimal rendering and
//! the meaning of `>>` and `/`.

use nybble_core::{eval, eval_float, f64_to_value, fixed, parse_base, Signedness, Value, Width};

use crate::settings::Settings;
use crate::theme::{self, ThemeMode};
use crate::widgets;

mod layout;
mod sections;

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
    /// Whether the "What's new" dialog is open.
    changelog_open: bool,
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
    /// detector across DPI transitions (see the DPI block in `ui`).
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

        // Show the "What's new" dialog once after an update: the running binary's
        // version differs from the one last seen, and we have notes for it. A
        // fresh install (no stored version) is not treated as an upgrade.
        let last_seen_version = storage.and_then(|s| s.get_string("last_seen_version"));
        let changelog_open = match &last_seen_version {
            Some(prev) => {
                prev != env!("CARGO_PKG_VERSION")
                    && crate::changelog::notes_for(env!("CARGO_PKG_VERSION")).is_some()
            }
            None => false,
        };

        // Register JetBrains Mono as the primary monospace font to match the design.
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "JetBrainsMono".to_owned(),
            egui::FontData::from_static(include_bytes!(
                "../../../../resources/fonts/JetBrainsMono-Regular.ttf"
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
            changelog_open,
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

    fn buffer_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Hex => &mut self.hex,
            Field::Dec => &mut self.dec,
            Field::Bin => &mut self.bin,
            Field::Oct => &mut self.oct,
            Field::Fixed => unreachable!(),
        }
    }

    fn on_field_edit(&mut self, field: Field) {
        if self.is_float_mode() {
            self.on_field_edit_float(field);
            return;
        }
        let text = self.buffer_mut(field).clone();
        let radix = field_radix(field);
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
                let text = self.buffer_mut(field).clone();
                let radix = field_radix(field);
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
    // Keep in step with `with_min_inner_size` in main.rs.
    const MIN: egui::Vec2 = egui::vec2(420.0, 460.0);
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

        // Per-monitor DPI scale. The app is Per-Monitor-V2 DPI-aware (see
        // crates/gui/build.rs), so pixels_per_point changes whenever the window
        // crosses to a screen with a different scale factor (e.g. a 4K panel at
        // 125% to an FHD panel at 100%) — letting egui re-render crisply at the
        // new DPI. The monitor handoff itself is handled by the vendored winit
        // (which backports the winit#4041 fix, see vendor/winit/NOTICE), so it
        // no longer ping-pongs.
        //
        // content_rect() is reported in logical points, so a ppp change makes
        // the size appear to jump even though the physical window is unchanged.
        // Arm the cooldown for a few frames so the resize detector below doesn't
        // misread that transient as a manual resize and demote the view to
        // Custom.
        let ppp = ui.ctx().pixels_per_point();
        if (ppp - self.last_ppp).abs() > f32::EPSILON {
            self.last_ppp = ppp;
            self.resize_cooldown = self.resize_cooldown.max(10);
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
                    let version_clicked = ui
                        .add(
                            egui::Label::new(
                                egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                                    .monospace()
                                    .weak()
                                    .small(),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .on_hover_text("What's new")
                        .clicked();
                    if version_clicked {
                        self.changelog_open = true;
                    }
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
                            if let Some(mode) = widgets::theme_icon_toggle(ui, self.theme_mode) {
                                self.theme_mode = mode;
                            }
                            if widgets::settings_icon_button(ui).clicked() {
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
                            } else if !self.updating
                                && self.update_rx.is_none()
                                && ui
                                    .button("Check for updates")
                                    .on_hover_text("Check GitHub Releases for a newer version")
                                    .clicked()
                            {
                                self.spawn_update_check(ui.ctx().clone());
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
        self.changelog_window(ui.ctx());
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
        // Mark the running version as seen so the "What's new" dialog only
        // appears once per upgrade.
        storage.set_string("last_seen_version", env!("CARGO_PKG_VERSION").to_owned());
        self.settings.save(storage);
    }
}

fn field_radix(field: Field) -> u32 {
    match field {
        Field::Hex => 16,
        Field::Dec => 10,
        Field::Bin => 2,
        Field::Oct => 8,
        Field::Fixed => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nybble_core::{Signedness, Value, Width};

    impl App {
        fn for_test() -> Self {
            let width = Width::new(32).unwrap();
            let mut app = Self {
                value: Value::new(0, width),
                width,
                sign: Signedness::Unsigned,
                frac_bits: 0,
                number_mode: NumberMode::Integer,
                float_value: 0.0,
                hex: String::new(),
                dec: String::new(),
                bin: String::new(),
                oct: String::new(),
                fixed_input: String::new(),
                expr: String::new(),
                history: Vec::new(),
                history_base: HistoryBase::Dec,
                expr_error: None,
                expr_focus_request: false,
                status: None,
                status_until: 0.0,
                flash_until: 0.0,
                value_just_changed: false,
                range_hi: 7,
                range_lo: 0,
                width_scrub_accum: 0.0,
                mode_toggle_anim: 0.0,
                theme_mode: ThemeMode::default(),
                view_mode: ViewMode::default(),
                settings: Settings::default(),
                settings_open: false,
                settings_tab: SettingsTab::default(),
                changelog_open: false,
                custom_size: None,
                resize_cooldown: 0,
                startup_resize_pending: false,
                last_ppp: 0.0,
                auto_check_updates: false,
                update_rx: None,
                update_available: None,
                updating: false,
            };
            app.refresh(None);
            app
        }
    }

    fn w(bits: u32) -> Width {
        Width::new(bits).unwrap()
    }

    // --- refresh ---

    #[test]
    fn refresh_populates_all_buffers() {
        let mut app = App::for_test();
        app.value = Value::new(0xDEAD_BEEF, w(32));
        app.refresh(None);
        assert_eq!(app.hex, "DEAD_BEEF");
        assert_eq!(app.dec, "3735928559");
        assert!(!app.bin.is_empty());
        assert!(!app.oct.is_empty());
    }

    #[test]
    fn refresh_skip_preserves_skipped_field() {
        let mut app = App::for_test();
        app.hex = "SENTINEL".to_string();
        app.value = Value::new(0xFF, w(32));
        app.refresh(Some(Field::Hex));
        assert_eq!(app.hex, "SENTINEL");
        assert_eq!(app.dec, "255");
    }

    #[test]
    fn refresh_float_mode_uses_float_value() {
        let mut app = App::for_test();
        app.set_number_mode(NumberMode::Float);
        app.float_value = 1.0;
        app.refresh(None);
        assert_eq!(app.dec, "1");
        // hex/bin/oct show the IEEE-754 pattern of 1.0_f64
        let expected_bits = f64_to_value(1.0_f64);
        assert_eq!(app.hex, expected_bits.to_hex());
    }

    // --- set_number_mode ---

    #[test]
    fn set_number_mode_int_to_float_seeds_from_unsigned_value() {
        let mut app = App::for_test();
        app.value = Value::new(42, w(32));
        app.sign = Signedness::Unsigned;
        app.set_number_mode(NumberMode::Float);
        assert!(app.is_float_mode());
        assert_eq!(app.float_value, 42.0);
    }

    #[test]
    fn set_number_mode_int_to_float_seeds_signed_interpretation() {
        let mut app = App::for_test();
        app.value = Value::new(0xFF, w(8)); // -1 in signed 8-bit
        app.sign = Signedness::Signed;
        app.set_number_mode(NumberMode::Float);
        assert_eq!(app.float_value, -1.0);
    }

    #[test]
    fn set_number_mode_float_to_int_preserves_integer_value() {
        let mut app = App::for_test();
        app.value = Value::new(99, w(32));
        app.set_number_mode(NumberMode::Float);
        app.float_value = 3.14;
        app.set_number_mode(NumberMode::Integer);
        assert!(!app.is_float_mode());
        assert_eq!(app.value.raw(), 99);
    }

    #[test]
    fn set_number_mode_noop_when_already_in_that_mode() {
        let mut app = App::for_test();
        app.value = Value::new(7, w(32));
        let before = app.value.raw();
        app.set_number_mode(NumberMode::Integer);
        assert_eq!(app.value.raw(), before);
    }

    // --- recall ---

    #[test]
    fn recall_integer_restores_mode_value_sign_and_expr() {
        let mut app = App::for_test();
        let entry = HistoryEntry {
            expr: "0xBEEF".to_string(),
            result: HistoryResult::Integer {
                value: Value::new(0xBEEF, w(16)),
                sign: Signedness::Signed,
            },
        };
        app.recall(entry);
        assert!(!app.is_float_mode());
        assert_eq!(app.value.raw(), 0xBEEF);
        assert_eq!(app.width.bits(), 16);
        assert_eq!(app.sign, Signedness::Signed);
        assert_eq!(app.expr, "0xBEEF");
        assert_eq!(app.hex, "BEEF");
    }

    #[test]
    fn recall_float_restores_mode_and_float_value() {
        let mut app = App::for_test();
        let entry = HistoryEntry {
            expr: "sqrt(2)".to_string(),
            result: HistoryResult::Float(std::f64::consts::SQRT_2),
        };
        app.recall(entry);
        assert!(app.is_float_mode());
        assert_eq!(app.float_value, std::f64::consts::SQRT_2);
        assert_eq!(app.expr, "sqrt(2)");
    }

    #[test]
    fn recall_integer_clamps_frac_bits_to_new_width() {
        let mut app = App::for_test();
        app.frac_bits = 32;
        let entry = HistoryEntry {
            expr: "1".to_string(),
            result: HistoryResult::Integer {
                value: Value::new(1, w(8)),
                sign: Signedness::Unsigned,
            },
        };
        app.recall(entry);
        assert!(app.frac_bits <= 8);
    }

    // --- on_field_edit ---

    #[test]
    fn on_field_edit_hex_parses_and_updates_other_buffers() {
        let mut app = App::for_test();
        app.hex = "DEADBEEF".to_string();
        app.on_field_edit(Field::Hex);
        assert_eq!(app.value.raw(), 0xDEAD_BEEF);
        assert!(app.status.is_none());
        assert_eq!(app.dec, "3735928559");
    }

    #[test]
    fn on_field_edit_dec_parses_correctly() {
        let mut app = App::for_test();
        app.dec = "255".to_string();
        app.on_field_edit(Field::Dec);
        assert_eq!(app.value.raw(), 255);
    }

    #[test]
    fn on_field_edit_bin_parses_correctly() {
        let mut app = App::for_test();
        app.bin = "1111".to_string();
        app.on_field_edit(Field::Bin);
        assert_eq!(app.value.raw(), 15);
    }

    #[test]
    fn on_field_edit_invalid_input_sets_status_error() {
        let mut app = App::for_test();
        app.hex = "ZZZZ".to_string();
        app.on_field_edit(Field::Hex);
        assert!(app.status.is_some());
    }

    #[test]
    fn on_field_edit_float_dec_updates_float_value() {
        let mut app = App::for_test();
        app.set_number_mode(NumberMode::Float);
        app.dec = "3.14".to_string();
        app.on_field_edit(Field::Dec);
        assert!((app.float_value - 3.14).abs() < 1e-10);
        assert!(app.status.is_none());
    }

    #[test]
    fn on_field_edit_float_hex_reinterprets_ieee754_bits() {
        let mut app = App::for_test();
        app.set_number_mode(NumberMode::Float);
        // IEEE-754 bits for 1.0_f64: 3FF0000000000000
        app.hex = "3FF0000000000000".to_string();
        app.on_field_edit(Field::Hex);
        assert_eq!(app.float_value, 1.0_f64);
    }
}
