# Component Inventory — `powercalc-gui`

_Generated: 2026-06-16 · Scan level: exhaustive_

egui is immediate-mode, so "components" are **UI sections** (methods on `App` that
draw a region each frame) and **reusable widgets** (free functions). This catalogs
them.

## Layout map

```
PowerCalc window
├── Header            heading "PowerCalc" + theme toggle button (Auto→Light→Dark)
└── Two columns
    ├── Left
    │   ├── EXPRESSION section   expression_centerpiece()
    │   └── HISTORY section      history_panel() → history_list()
    └── Right
        ├── CURRENT VALUE        current_value_compact()
        ├── FORMAT               controls_bar() + fixed_point()
        └── BITS (MSB → LSB)     bit_grid()  [reusable widget]
```

## Sections (methods on `App`, in `app.rs`)

| Section | Method | Category | Description |
|---------|--------|----------|-------------|
| Expression centerpiece | `expression_centerpiece` | Input / Action | Monospace 22px expression line + accent "Evaluate" button; Enter or click evaluates. Op buttons (`AND OR XOR NOT SHL SHR ( ) + - * / ans`) append tokens; "Clear" empties the line. Inline red error under the field on invalid eval. |
| History panel | `history_panel` | Display / Navigation | Collapsing "HISTORY" header with a base selector (All/HEX/DEC/BIN/OCT) and a "Clear" button. |
| History list | `history_list` | Display / Action | Scrollable (max 240px), newest-first cards. Each shows the expression (accent, click to recall) and its result rendered in the selected base(s); empty-state hint when no history. |
| Controls bar | `controls_bar` | Input | Width presets 8/16/32/64 (selectable) + custom slider 1..=128; signed/unsigned toggle. |
| Current value (compact) | `current_value_compact` | Input / Display | Base selector; the focused base shown large (20px mono) and editable (drives all views); the other three as small click-to-copy lines. |
| Fixed-point | `fixed_point` | Input | `Qm.n` label, frac-bits slider (0..=width), and a "Real" text field that writes back into the value. |
| Bit grid host | `bit_grid` | Input / Display | Wraps the `bit_grid` widget; applies the returned toggled value. |

## Reusable widgets (`widgets/`)

| Widget | Location | Signature | Description |
|--------|----------|-----------|-------------|
| `bit_grid` | `widgets/bitgrid.rs` | `bit_grid(ui, value, accent) -> Option<Value>` | MSB→LSB clickable 0/1 toggle grid; rows are a multiple of 8 wide, adapt to available width, gap every nibble, each row labeled with its top bit index. Returns the value with the clicked bit toggled. |

## Shared UI helpers (free fns in `app.rs`)

| Helper | Purpose |
|--------|---------|
| `App::section` | Wrap content in a rounded, filled "card" frame. |
| `field_label(Field)` | Label string for a base field (HEX/DEC/BIN/OCT/FIX). |
| `section_label(ui, text)` | Small weak section heading. |
| `mini_value_line(ui, label, text)` | Small weak click-to-copy base line. |
| `value_lines` / `value_line` | Render a history result in one/all bases, click-to-copy. |
| `parse_base(text, radix, width, sign)` | GUI base-field parser (prefixes, `_`/whitespace stripped, signed `-`). |

## Theme components (`theme.rs`)

| Item | Purpose |
|------|---------|
| `ThemeMode { Auto, Light, Dark }` | Persisted theme preference; `label`/`next`/`key`/`from_key`. |
| `apply(ctx, mode)` | Resolve mode → `egui::Theme`, install built `Style`. |
| `Palette` / `palette(theme)` | Indigo-accented dark/light color set. |
| `build_style(theme)` | Visuals (rounded widgets, accent strokes), spacing, typography. |
| `card_fill` / `accent` / `on_accent` | Color accessors used by sections/widgets. |

## State enums (`app.rs`)

| Type | Values | Role |
|------|--------|------|
| `Field` | Hex, Dec, Bin, Oct, Fixed | Which surface is focused/being edited (drives `refresh` skip). |
| `HistoryBase` | All, Hex, Dec, Bin, Oct | Which base(s) history rows display; persisted. |
| `HistoryEntry` | `{ expr, value, sign }` | One recorded evaluation, with its sign mode. |

## Notes

- No automated UI tests; all logic-bearing code is in `powercalc-core`.
- The accent indigo is applied explicitly (Evaluate button, selections, set bits)
  because egui's "strong" text color reuses `widgets.active.fg_stroke`.
