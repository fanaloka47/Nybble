//! User-configurable view and copy settings.
//!
//! This module owns everything the Settings modal can change: which panels are
//! shown and in what order, which fields appear inside them, and how values are
//! transformed when copied to the clipboard. The whole thing is one [`Settings`]
//! value persisted via eframe storage; it is deliberately shaped as a single
//! snapshot so a future "named layouts" feature can store a `Vec<Settings>`
//! without reworking anything here.
//!
//! Serialization follows the same flat string key/value convention the rest of
//! the app uses (`key()` / `from_key()`), so it slots straight into
//! `App::new` / `App::save`.

/// The reorderable, toggleable panels below the expression box.
///
/// The expression centerpiece is always shown full-width and is intentionally
/// *not* a member here — it is not subject to layout settings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Panel {
    Value,
    Bits,
    Format,
    Interpret,
    History,
}

impl Panel {
    /// All panels in their factory-default order.
    pub const DEFAULT_ORDER: [Panel; 5] = [
        Panel::Value,
        Panel::Bits,
        Panel::Format,
        Panel::Interpret,
        Panel::History,
    ];

    /// Stable index into the `panel_enabled` array.
    pub fn index(self) -> usize {
        match self {
            Panel::Value => 0,
            Panel::Bits => 1,
            Panel::Format => 2,
            Panel::Interpret => 3,
            Panel::History => 4,
        }
    }

    /// Coarse relative height, used to balance the two-column layout.
    pub fn weight(self) -> f32 {
        match self {
            Panel::Value => 4.0,
            Panel::Bits => 4.0,
            Panel::History => 3.0,
            Panel::Interpret => 3.0,
            Panel::Format => 2.0,
        }
    }

    /// Human-readable name for the settings UI.
    pub fn label(self) -> &'static str {
        match self {
            Panel::Value => "Current value",
            Panel::Bits => "Bits",
            Panel::Format => "Format",
            Panel::Interpret => "Interpret",
            Panel::History => "History",
        }
    }

    /// Stable key for persistence.
    pub fn key(self) -> &'static str {
        match self {
            Panel::Value => "value",
            Panel::Bits => "bits",
            Panel::Format => "format",
            Panel::Interpret => "interpret",
            Panel::History => "history",
        }
    }

    pub fn from_key(s: &str) -> Option<Panel> {
        match s {
            "value" => Some(Panel::Value),
            "bits" => Some(Panel::Bits),
            "format" => Some(Panel::Format),
            "interpret" => Some(Panel::Interpret),
            "history" => Some(Panel::History),
            _ => None,
        }
    }
}

/// How a base rendering is transformed before landing on the clipboard.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CopyOptions {
    /// Prepend `0x`/`0b`/`0o` to HEX/BIN/OCT copies.
    pub prepend_prefix: bool,
    /// Keep the width-padding leading zeros (e.g. `00FF` vs `FF`).
    pub keep_leading_zeros: bool,
    /// Keep the `_` group separators (e.g. `DEAD_BEEF` vs `DEADBEEF`).
    pub keep_separators: bool,
}

impl Default for CopyOptions {
    fn default() -> Self {
        // Reproduces the historical behaviour: prefix on, zeros kept,
        // separators stripped.
        CopyOptions {
            prepend_prefix: true,
            keep_leading_zeros: true,
            keep_separators: false,
        }
    }
}

impl CopyOptions {
    /// Transform a displayed base string (`label` is "HEX"/"DEC"/"BIN"/"OCT")
    /// into its clipboard form per these options.
    pub fn apply(&self, label: &str, display: &str) -> String {
        let prefix = match label {
            "HEX" => Some("0x"),
            "BIN" => Some("0b"),
            "OCT" => Some("0o"),
            _ => None, // DEC (and any non-base label) has no prefix
        };

        // Separators are presentation-only; drop them unless asked to keep.
        // HEX/BIN/OCT use '_'; DEC uses '\'' as thousands separator.
        let mut body: String = if self.keep_separators {
            display.to_owned()
        } else {
            display.chars().filter(|&c| c != '_' && c != '\'').collect()
        };

        // Leading zeros only ever appear in the padded bit bases. DEC is never
        // zero-padded, so this is a no-op there (and we never strip a `-` sign).
        if !self.keep_leading_zeros && prefix.is_some() {
            body = strip_leading_zeros(&body);
        }

        match prefix {
            Some(p) if self.prepend_prefix => format!("{p}{body}"),
            _ => body,
        }
    }
}

/// Strip leading `'0'` characters, keeping at least one digit and any `_`
/// separators that survive in front. `"00FF" -> "FF"`, `"00" -> "0"`.
fn strip_leading_zeros(s: &str) -> String {
    let trimmed = s.trim_start_matches(['0', '_']);
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// The complete, persisted view/copy configuration.
#[derive(Clone)]
pub struct Settings {
    /// Every panel, in display order. `panel_enabled` says which are shown.
    pub panel_order: Vec<Panel>,
    /// Per-panel visibility, indexed by [`Panel::index`].
    pub panel_enabled: [bool; 5],

    // Current-value base fields.
    pub show_hex: bool,
    pub show_dec: bool,
    pub show_bin: bool,
    pub show_oct: bool,

    // Interpret-panel sub-blocks.
    pub show_fixed_point: bool,
    pub show_bit_slicer: bool,

    /// After evaluating an expression, replace the field's contents with the
    /// result rendered in decimal, ready to build the next expression on. Off
    /// by default — the field normally keeps the typed expression.
    pub result_to_expression: bool,

    pub copy: CopyOptions,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            panel_order: Panel::DEFAULT_ORDER.to_vec(),
            panel_enabled: [true; 5],
            show_hex: true,
            show_dec: true,
            show_bin: true,
            show_oct: true,
            show_fixed_point: true,
            show_bit_slicer: true,
            result_to_expression: false,
            copy: CopyOptions::default(),
        }
    }
}

impl Settings {
    pub fn is_panel_enabled(&self, panel: Panel) -> bool {
        self.panel_enabled[panel.index()]
    }

    pub fn set_panel_enabled(&mut self, panel: Panel, on: bool) {
        self.panel_enabled[panel.index()] = on;
    }

    /// Move the panel at `idx` one slot earlier in the order (no-op at the top).
    pub fn move_up(&mut self, idx: usize) {
        if idx > 0 && idx < self.panel_order.len() {
            self.panel_order.swap(idx - 1, idx);
        }
    }

    /// Move the panel at `idx` one slot later in the order (no-op at the end).
    pub fn move_down(&mut self, idx: usize) {
        if idx + 1 < self.panel_order.len() {
            self.panel_order.swap(idx, idx + 1);
        }
    }

    // --- Persistence ----------------------------------------------------

    /// Load from eframe storage, falling back to defaults for any missing or
    /// malformed key.
    pub fn load(storage: &dyn eframe::Storage) -> Settings {
        let mut s = Settings::default();

        // Order: parse the stored comma list, then append any panels that
        // weren't present (forward-compat if new panels are ever added).
        if let Some(order) = storage.get_string("panel_order") {
            let mut parsed: Vec<Panel> = order.split(',').filter_map(Panel::from_key).collect();
            for p in Panel::DEFAULT_ORDER {
                if !parsed.contains(&p) {
                    parsed.push(p);
                }
            }
            if !parsed.is_empty() {
                s.panel_order = parsed;
            }
        }

        let flag = |key: &str, default: bool| {
            storage
                .get_string(key)
                .map(|v| v != "false")
                .unwrap_or(default)
        };

        for p in Panel::DEFAULT_ORDER {
            s.panel_enabled[p.index()] = flag(&format!("panel_{}", p.key()), true);
        }
        s.show_hex = flag("field_hex", true);
        s.show_dec = flag("field_dec", true);
        s.show_bin = flag("field_bin", true);
        s.show_oct = flag("field_oct", true);
        s.show_fixed_point = flag("show_fixed_point", true);
        s.show_bit_slicer = flag("show_bit_slicer", true);
        s.result_to_expression = flag("result_to_expression", false);
        s.copy.prepend_prefix = flag("copy_prefix", true);
        s.copy.keep_leading_zeros = flag("copy_leading_zeros", true);
        s.copy.keep_separators = flag("copy_separators", false);
        s
    }

    /// Write every field to eframe storage.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        let order = self
            .panel_order
            .iter()
            .map(|p| p.key())
            .collect::<Vec<_>>()
            .join(",");
        storage.set_string("panel_order", order);

        let mut put = |key: &str, val: bool| {
            storage.set_string(key, if val { "true" } else { "false" }.to_owned());
        };
        for p in Panel::DEFAULT_ORDER {
            put(&format!("panel_{}", p.key()), self.panel_enabled[p.index()]);
        }
        put("field_hex", self.show_hex);
        put("field_dec", self.show_dec);
        put("field_bin", self.show_bin);
        put("field_oct", self.show_oct);
        put("show_fixed_point", self.show_fixed_point);
        put("show_bit_slicer", self.show_bit_slicer);
        put("result_to_expression", self.result_to_expression);
        put("copy_prefix", self.copy.prepend_prefix);
        put("copy_leading_zeros", self.copy.keep_leading_zeros);
        put("copy_separators", self.copy.keep_separators);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_default_matches_legacy() {
        // The old clipboard_form: strip separators, keep zeros, add prefix.
        let c = CopyOptions::default();
        assert_eq!(c.apply("HEX", "DEAD_BEEF"), "0xDEADBEEF");
        assert_eq!(c.apply("BIN", "0000_1101"), "0b00001101");
        assert_eq!(c.apply("OCT", "377"), "0o377");
        assert_eq!(c.apply("DEC", "255"), "255");
    }

    #[test]
    fn copy_no_prefix() {
        let c = CopyOptions {
            prepend_prefix: false,
            ..Default::default()
        };
        assert_eq!(c.apply("HEX", "DEAD_BEEF"), "DEADBEEF");
        assert_eq!(c.apply("DEC", "42"), "42");
    }

    #[test]
    fn copy_strip_leading_zeros() {
        let c = CopyOptions {
            keep_leading_zeros: false,
            ..Default::default()
        };
        assert_eq!(c.apply("HEX", "05"), "0x5");
        assert_eq!(c.apply("HEX", "0000_00FF"), "0xFF");
        assert_eq!(c.apply("BIN", "0000_0000"), "0b0");
        // DEC has no padding, so it's untouched (and the sign is preserved).
        assert_eq!(c.apply("DEC", "0"), "0");
    }

    #[test]
    fn copy_keep_separators() {
        let c = CopyOptions {
            keep_separators: true,
            ..Default::default()
        };
        assert_eq!(c.apply("HEX", "DEAD_BEEF"), "0xDEAD_BEEF");
    }

    #[test]
    fn reorder_bounds() {
        let mut s = Settings::default();
        s.move_up(0); // no-op at top
        assert_eq!(s.panel_order[0], Panel::Value);
        s.move_down(0);
        assert_eq!(s.panel_order[0], Panel::Bits);
        assert_eq!(s.panel_order[1], Panel::Value);
        let last = s.panel_order.len() - 1;
        s.move_down(last); // no-op at end
        assert_eq!(s.panel_order[last], Panel::History);
    }
}
