# Component Inventory — `powercalc-gui`

_Generated: 2026-06-16 · Updated: 2026-06-23 (full rescan) · Scan level: exhaustive_

egui is immediate-mode, so "components" are **UI sections** (methods on `App` that
draw a region each frame) and **reusable widgets** (free functions). This catalogs
them.

## Layout map

```
PowerCalc window
├── Header              heading "PowerCalc" + View button (Compact→Full→Custom)
│                       + Theme button (Auto→Light→Dark)
├── EXPRESSION          expression_centerpiece()   [full width]
└── Body (two columns when wide; single stack when narrow / Compact)
    ├── CURRENT VALUE   current_value_compact()
    ├── BITS · MSB→LSB  bits_section() → bit_grid()  [hidden in float mode]
    ├── FORMAT          format_section()             [integer controls hidden in float mode]
    └── HISTORY         history_panel() → history_list()
```

## Sections (methods on `App`, in `app.rs`)

| Section | Method | Category | Description |
|---------|--------|----------|-------------|
| Expression centerpiece | `expression_centerpiece` | Input / Action | An **int / float** mode toggle, then a monospace 22px expression line + accent "↵" Evaluate button; Enter or click evaluates and pushes to history. Inline red error under the field on invalid eval (cleared as you type). |
| History panel | `history_panel` | Display / Navigation | Collapsing "HISTORY" header (default open) with a base selector (All/HEX/DEC/BIN/OCT) and a "Clear" button. |
| History list | `history_list` | Display / Action | Scrollable, newest-first cards sized to ~5 entries (height derived from the active base filter). Each shows the expression (accent, click to recall) and its result in the selected base(s); float entries render the decimal plus the f64 bit pattern. Empty-state hint when no history. |
| Format section | `format_section` | Input | Width presets 8/16/32/64 + a draggable "{n}-bit ↔" scrubber (3px/bit); signed/unsigned toggle; the fixed-point split; the bit-range extractor. In float mode shows a note instead (controls don't apply). |
| Current value (compact) | `current_value_compact` | Input / Display | Base selector; the focused base shown large (20px mono) and editable (drives all views); the other three as small click-to-copy lines. |
| Fixed-point | `fixed_point` | Input | `Qm.n` label, a click/drag fill bar that sets the fractional/integer split, and a "real" text field that writes back into the value. |
| Bit range | `bit_range` | Input / Display | Pick `bits[hi:lo]` (two `DragValue`s, normalized so hi ≥ lo); reads the slice back in hex/dec/bin in a click-to-copy card. |
| Bits section | `bits_section` | Input / Display | Wraps the `bit_grid` widget; applies the returned toggled value. |
| Toast | `toast` | Display | Bottom-anchored transient message (copies, parse errors); auto-dismisses (errors persist until the next success). |

## Reusable widgets (`widgets/`)

| Widget | Location | Signature | Description |
|--------|----------|-----------|-------------|
| `bit_grid` | `widgets/bitgrid.rs` | `bit_grid(ui, value, accent) -> Option<Value>` | MSB→LSB bit view: a header (top bit index · set count · bit 0), a clickable thin bit bar, and clickable 0/1 cells in non-breaking nibble groups (as many groups per row as fit), each group labeled underneath with its lowest bit index. Returns the value with the clicked bit toggled. |

## Shared UI helpers (free fns in `app.rs`)

| Helper | Purpose |
|--------|---------|
| `App::section` | Wrap content in a rounded, filled "card" frame. |
| `field_label(Field)` | Label string for a base field (HEX/DEC/BIN/OCT/FIX). |
| `paint_input_frame(ui, rect, accent)` | Accent outline + left-edge stripe marking a field as editable. |
| `section_label(ui, text)` | Small weak section heading. |
| `mini_value_line(ui, label, text)` | Small weak click-to-copy base line. |
| `value_lines` / `value_line` | Render an integer history result in one/all bases, click-to-copy. |
| `float_value_lines` | Render a float history result (decimal + f64 bit pattern). |
| `parse_base(text, radix, width, sign)` / `strip_radix_prefix` | GUI base-field parser (prefixes, `_`/whitespace stripped, signed `-`). |

## Theme components (`theme.rs`)

| Item | Purpose |
|------|---------|
| `ThemeMode { Auto, Light, Dark }` | Persisted theme preference; `label`/`next`/`key`/`from_key`. |
| `apply(ctx, mode)` | Resolve mode → `egui::Theme`, install built `Style`. |
| `Palette` / `palette(theme)` | Indigo-accented dark/light color set. |
| `build_style(theme)` | Visuals (rounded widgets, accent strokes), spacing, typography, `selectable_labels = false`. |
| `card_fill` / `accent` / `on_accent` | Color accessors used by sections/widgets. |

## State enums / structs (`app.rs`)

| Type | Values | Role |
|------|--------|------|
| `Field` | Hex, Dec, Bin, Oct, Fixed | Which surface is focused/being edited (drives `refresh` skip). |
| `NumberMode` | Integer, Float | Integer vs full-precision f64 evaluation; persisted. |
| `ViewMode` | Compact, Full, Custom | Window-sizing mode (420×700 / 880×760 / last manual size); persisted. |
| `HistoryBase` | All, Hex, Dec, Bin, Oct | Which base(s) history rows display; persisted. |
| `HistoryResult` | `Integer { value, sign }` \| `Float(f64)` | A result in its originating mode. |
| `HistoryEntry` | `{ expr, result }` | One recorded evaluation. |

## Notes

- No automated UI tests; all logic-bearing code is in `powercalc-core`.
- The accent indigo is applied explicitly (Evaluate button, selections, set bits)
  because egui's "strong" text color reuses `widgets.active.fg_stroke`.
- Float mode hides the bit grid and integer Format controls and reinterprets the
  value as an `f64` everywhere.
</content>
