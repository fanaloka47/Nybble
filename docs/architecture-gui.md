# Architecture — `powercalc-gui`

_Generated: 2026-06-16 · Part type: desktop · Scan level: exhaustive_

## Executive summary

`powercalc-gui` is the desktop application: a thin
[`eframe`](https://crates.io/crates/eframe) / [`egui`](https://github.com/emilk/egui)
0.34 immediate-mode UI over `powercalc-core`. It owns presentation and UI state;
all numeric work is delegated to the core library. The compiled binary is named
`powercalc`.

## Technology stack

| Category      | Detail |
|---------------|--------|
| Language      | Rust, edition 2021 |
| Crate type    | `bin` (`[[bin]] name = "powercalc"`, `path = src/main.rs`) |
| GUI framework | `eframe` 0.34 (`default-features = false`), `egui` 0.34 |
| eframe features | `glow`, `default_fonts`, `persistence`, `wayland`, `x11` |
| Render backend | Glow (OpenGL) — chosen for WSLg reliability and lighter deps |
| Notable omission | `accesskit` disabled (it needs a D-Bus session, often absent under WSL) |
| Local dependency | `powercalc-core` (path `../core`) |

## Architecture pattern

Standard **eframe immediate-mode** app: a single `App` struct holds all state and
implements `eframe::App`. The `ui()` method re-runs every frame, reading state and
drawing widgets; user interactions mutate state immediately. There is no retained
widget tree, no MVC, no async.

### Entry point (`main.rs`)

`main()` configures `eframe::NativeOptions` — Glow renderer, 760×720 default
window, 520×480 minimum — and calls `eframe::run_native("PowerCalc", …)`,
constructing the `App` from the `CreationContext` (which carries persisted
storage).

### Application state (`app.rs`)

`struct App` is the single source of UI truth:

- **Canonical numeric state:** `value: Value`, `width: Width`, `sign: Signedness`,
  `frac_bits: u32`, `custom_width: u32`.
- **Editable text buffers** (one per surface): `hex`, `dec`, `bin`, `oct`,
  `fixed_input`, `expr`.
- **View/UI state:** `focus_base: Field` (which base is shown large/editable),
  `history: Vec<HistoryEntry>`, `history_base: HistoryBase`, `expr_error`,
  `status`, `theme_mode: ThemeMode`.

Supporting enums/structs: `Field { Hex, Dec, Bin, Oct, Fixed }` (which surface is
being edited, used to skip refreshing the field the user is typing in);
`HistoryBase { All, Hex, Dec, Bin, Oct }` (which base[s] history rows show, with
`label`/`key`/`from_key` for persistence); `HistoryEntry { expr, value, sign }`
(captures the sign mode at evaluation time so the decimal stays faithful on recall).

### Core state-update methods

| Method | Responsibility |
|--------|----------------|
| `refresh(skip)` | Rewrite every text buffer from `value`, except the `skip` field being edited. |
| `format_fixed()` | Render the value as a fixed-point real (via `core::fixed`). |
| `set_width(bits)` | Clamp width, re-mask value, clamp `frac_bits`, refresh. |
| `set_sign(sign)` | Change signedness, refresh (decimal re-renders). |
| `on_field_edit(field)` | Parse a base buffer → new `value`, refresh others; set `status` on error. |
| `on_fixed_edit()` | Parse the real-number buffer → `value` via `fixed::from_real`. |
| `eval_expr()` | Evaluate `expr` via `core::eval`, push to history, refresh; set `expr_error` on failure. |
| `push_history(expr, value)` | Append a `HistoryEntry`, trimming to `MAX_HISTORY = 200`. |
| `recall(entry)` | Restore a history entry's value/width/sign/expr text. |

### Layout (`App::ui`)

Each frame: apply the theme, then a `CentralPanel` → vertical `ScrollArea` →
header (title + theme toggle button) → a **two-column** layout:

- **Left column:** the expression centerpiece (text line + Evaluate button + op
  buttons), then the history panel.
- **Right column:** the compact current-value view (base selector + large editable
  base + the other three as click-to-copy lines), then the FORMAT card
  (width presets/slider, sign toggle, fixed-point controls), then the BITS card
  (the interactive bit grid).

A `status` error message renders at the bottom when set. Sections are wrapped in
rounded filled "cards" by the `App::section` helper.

### Persistence

`App::new` reads `theme_mode` and `history_base` from eframe storage (falling back
to defaults); `App::save` writes them back. Only these two preferences are
persisted — values and history are session-only.

### Theming (`theme.rs`)

`ThemeMode { Auto, Light, Dark }` (persisted; `Auto` follows the OS, defaulting to
Dark when unknown). `apply(ctx, mode)` resolves the mode to an `egui::Theme` and
installs a `Style` built by `build_style`. A `Palette` struct defines a cohesive
indigo-accented dark/light color set (backgrounds, surfaces, widgets, text,
borders, accent + soft accent + on-accent). The module exposes helpers
`card_fill`, `accent`, `on_accent`. Note in code: `widgets.active.fg_stroke`
doubles as egui's app-wide "strong" text color, so it is kept legible and the
accent is applied explicitly where needed.

### Widgets (`widgets/`)

`widgets/mod.rs` re-exports `bit_grid`. `widgets/bitgrid.rs::bit_grid(ui, value,
accent) -> Option<Value>` renders the value's bits as clickable 0/1 toggle
buttons, **MSB→LSB**, in rows whose length adapts to available width (always a
multiple of 8 so byte boundaries align, with a gap every nibble). Each row is
prefixed with its highest bit index. Clicking a bit returns the value with that
bit toggled (else `None`). Helpers: `bits_per_row` (responsive layout math),
`contrast_text` (luminance-based black/white choice over the accent fill).

## Free helper functions (`app.rs`)

`field_label`, `section_label`, `mini_value_line` (click-to-copy small base line),
`value_lines` / `value_line` (history result rendering, click-to-copy), and
`parse_base(text, radix, width, sign)` — the GUI's own base-field parser (strips
whitespace/`_`, accepts optional `0x`/`0b`/`0o` prefixes, and a leading `-` only in
signed decimal mode). Empty input parses to zero.

## Testing strategy

The GUI crate has **no automated tests** — by design, all testable logic lives in
`powercalc-core`. GUI verification is manual (`cargo run -p powercalc-gui`); see
the verification steps in [`powercalc-plan.md`](./powercalc-plan.md) and the
[Development Guide](./development-guide.md).

## Component inventory

See [Component Inventory — GUI](./component-inventory-gui.md) for the catalog of
UI sections and widgets. Core integration details are in
[Integration Architecture](./integration-architecture.md).
