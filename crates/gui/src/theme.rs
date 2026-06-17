//! Modern theming: a refined dark/light palette with an indigo accent, rounded
//! widgets, generous spacing and tuned typography.
//!
//! The active theme can follow the OS (`Auto`) or be pinned to `Light`/`Dark`.
//! [`apply`] is called once per frame: it resolves the mode to a concrete
//! [`egui::Theme`] and installs the matching [`egui::Style`].

use egui::style::WidgetVisuals;
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, Style, TextStyle, Theme, Visuals,
};

/// User-facing theme preference, persisted across runs.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Auto,
    Light,
    Dark,
}

impl ThemeMode {
    /// Short label for the toggle button.
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Auto => "Auto",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }

    /// Cycle Auto → Light → Dark → Auto.
    pub fn next(self) -> ThemeMode {
        match self {
            ThemeMode::Auto => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Auto,
        }
    }

    /// Stable key for persistence.
    pub fn key(self) -> &'static str {
        match self {
            ThemeMode::Auto => "auto",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }

    pub fn from_key(s: &str) -> Option<ThemeMode> {
        match s {
            "auto" => Some(ThemeMode::Auto),
            "light" => Some(ThemeMode::Light),
            "dark" => Some(ThemeMode::Dark),
            _ => None,
        }
    }
}

/// Resolve `mode` to a concrete theme and install the matching style.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let resolved = match mode {
        ThemeMode::Light => Theme::Light,
        ThemeMode::Dark => Theme::Dark,
        ThemeMode::Auto => ctx
            .input(|i| i.raw.system_theme)
            .unwrap_or(Theme::Dark),
    };
    ctx.set_theme(resolved);
    ctx.set_global_style(build_style(resolved));
}

/// Fill color for the section "cards".
pub fn card_fill(ctx: &egui::Context) -> Color32 {
    palette(ctx.theme()).surface
}

/// Accent color for the current theme (set bits, highlights).
pub fn accent(ctx: &egui::Context) -> Color32 {
    palette(ctx.theme()).accent
}

/// Legible text color to place on top of the accent fill.
pub fn on_accent(ctx: &egui::Context) -> Color32 {
    palette(ctx.theme()).on_accent
}

/// A cohesive set of colors for one theme.
struct Palette {
    dark: bool,
    bg: Color32,
    surface: Color32,
    input: Color32,
    widget: Color32,
    widget_hover: Color32,
    text: Color32,
    text_strong: Color32,
    border: Color32,
    accent: Color32,
    accent_soft: Color32,
    on_accent: Color32,
}

fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => Palette {
            dark: true,
            bg: Color32::from_rgb(15, 17, 21),
            surface: Color32::from_rgb(23, 26, 33),
            input: Color32::from_rgb(28, 32, 41),
            widget: Color32::from_rgb(37, 42, 54),
            widget_hover: Color32::from_rgb(48, 54, 68),
            text: Color32::from_rgb(226, 229, 238),
            text_strong: Color32::from_rgb(245, 247, 250),
            border: Color32::from_rgb(45, 51, 64),
            accent: Color32::from_rgb(129, 140, 248), // indigo-400
            accent_soft: Color32::from_rgba_unmultiplied(129, 140, 248, 90),
            on_accent: Color32::from_rgb(15, 17, 21),
        },
        Theme::Light => Palette {
            dark: false,
            bg: Color32::from_rgb(247, 248, 250),
            surface: Color32::from_rgb(255, 255, 255),
            input: Color32::from_rgb(255, 255, 255),
            widget: Color32::from_rgb(237, 239, 244),
            widget_hover: Color32::from_rgb(226, 230, 238),
            text: Color32::from_rgb(28, 32, 40),
            text_strong: Color32::from_rgb(17, 20, 26),
            border: Color32::from_rgb(214, 218, 226),
            accent: Color32::from_rgb(79, 70, 229), // indigo-600
            accent_soft: Color32::from_rgba_unmultiplied(79, 70, 229, 55),
            on_accent: Color32::from_rgb(255, 255, 255),
        },
    }
}

fn build_style(theme: Theme) -> Style {
    let p = palette(theme);
    let mut style = Style::default();

    // --- Visuals ---
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.dark_mode = p.dark;
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = p.input;
    v.faint_bg_color = if p.dark {
        Color32::from_rgb(28, 32, 41)
    } else {
        Color32::from_rgb(242, 243, 246)
    };
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.text);

    let radius = CornerRadius::same(8);
    v.widgets.noninteractive = WidgetVisuals {
        bg_fill: p.surface,
        weak_bg_fill: p.surface,
        bg_stroke: Stroke::new(1.0, p.border),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, p.text),
        expansion: 0.0,
    };
    v.widgets.inactive = WidgetVisuals {
        bg_fill: p.widget,
        weak_bg_fill: p.widget,
        bg_stroke: Stroke::NONE,
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, p.text),
        expansion: 0.0,
    };
    v.widgets.hovered = WidgetVisuals {
        bg_fill: p.widget_hover,
        weak_bg_fill: p.widget_hover,
        bg_stroke: Stroke::new(1.0, p.accent),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, p.text),
        expansion: 1.0,
    };
    // NB: `active.fg_stroke` doubles as the app-wide "strong" text color
    // (egui's `strong_text_color()` reads it), so it must stay a legible text
    // color — not the on-accent color. The accent is applied explicitly where
    // needed (Evaluate button, selections, bit grid).
    v.widgets.active = WidgetVisuals {
        bg_fill: p.widget_hover,
        weak_bg_fill: p.widget_hover,
        bg_stroke: Stroke::new(1.0, p.accent),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, p.text_strong),
        expansion: 1.0,
    };
    v.widgets.open = WidgetVisuals {
        bg_fill: p.widget,
        weak_bg_fill: p.widget,
        bg_stroke: Stroke::new(1.0, p.border),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, p.text),
        expansion: 0.0,
    };
    v.override_text_color = None;
    style.visuals = v;

    // --- Interaction ---
    style.interaction.selectable_labels = false;

    // --- Spacing ---
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.window_margin = Margin::same(14);

    // --- Typography ---
    style.text_styles = [
        (TextStyle::Heading, FontId::new(24.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(15.0, FontFamily::Monospace)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
    ]
    .into();

    style
}
