//! Panel visibility and column-balancing logic for [`App`].

use super::App;
use crate::settings::Panel;

impl App {
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
}
