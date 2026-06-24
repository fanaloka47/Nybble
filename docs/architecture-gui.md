# Architecture — `nybble-gui`

_Generated: 2026-06-16 · Updated: 2026-06-23 (full rescan) · Part type: desktop · Scan level: exhaustive_

## Executive summary

`nybble-gui` is the desktop application: a thin
[`eframe`](https://crates.io/crates/eframe) / [`egui`](https://github.com/emilk/egui)
0.34 immediate-mode UI over `nybble-core`. It owns presentation and UI state;
all numeric work is delegated to the core library. The compiled binary is named
`nybble`.

## Technology stack

| Category      | Detail |
|---------------|--------|
| Language      | Rust, edition 2021 |
| Crate type    | `bin` (`[[bin]] name = "nybble"`, `path = src/main.rs`) |
| GUI framework | `eframe` 0.34 (`default-features = false`), `egui` 0.34 |
| eframe features | `glow`, `default_fonts`, `persistence`; `x11` added only under `cfg(unix)` |
| Render backend | Glow (OpenGL) — chosen for WSLg reliability and lighter deps |
| Notable omissions | `accesskit` disabled (needs D-Bus, often absent under WSL); the `wayland` feature was **dropped** (it crashed on launch under WSL) |
| Optional feature | `screenshot` — enables eframe's `__screenshot`; with it on, `EFRAME_SCREENSHOT_TO=/path.png` saves a screenshot and exits |
| Local dependency | `nybble-core` (path `../core`) |

## Architecture pattern

Standard **eframe immediate-mode** app: a single `App` struct holds all state and
implements `eframe::App`. The `ui()` method re-runs every frame, reading state and
drawing widgets; user interactions mutate state immediately. There is no retained
widget tree, no MVC, no async.

### Entry point (`main.rs`)

`main()` configures `eframe::NativeOptions` — Glow renderer, 760×720 default
window, 520×480 minimum — and calls `eframe::run_native("nybble", …)`,
constructing the `App` from the `CreationContext` (which carries persisted
storage). Two niceties:

- `PC_SIZE=WIDTHxHEIGHT` overrides the initial window size (handy for reproducing
  a layout bug at the exact reported size).
- On non-debug builds a `windows_subsystem = "windows"` attribute prevents a
  console window from appearing behind the app on Windows; left off in debug so
  stderr / `PC_DEBUG` output stays visible.

### Application state (`app.rs`)

`struct App` is the single source of UI truth. Key fields:

- **Canonical numeric state:** `value: Value`, `width: Width`, `sign: Signedness`,
  `frac_bits: u32`, and — for float mode — `float_value: f64`. `number_mode:
  NumberMode` selects which is live.
- **Editable text buffers** (one per surface): `hex`, `dec`, `bin`, `oct`,
  `fixed_input`, `expr`.
- **View/UI state:** `focus_base: Field` (which base is shown large/editable),
  `history: Vec<HistoryEntry>`, `history_base: HistoryBase`, `expr_error`,
  `status` + `status_until` (transient toast), `range_hi`/`range_lo` (the
  `bits[hi:lo]` extractor), `width_scrub_accum` (sub-pixel accumulator for the
  width drag-scrubber), `theme_mode: ThemeMode`, `view_mode: ViewMode`,
  `custom_size`, `resize_cooldown`, `startup_resize_pending`.

Supporting enums/structs:

- `Field { Hex, Dec, Bin, Oct, Fixed }` — which surface is being edited (used to
  skip refreshing the field the user is typing in).
- `NumberMode { Integer, Float }` — integer (default, width-bound two's complement)
  vs. full-precision `f64`. Resolved through the single `App::is_float_mode`
  accessor; persisted via `key`/`from_key`.
- `ViewMode { Compact, Full, Custom }` — window-sizing mode. `Compact` forces a
  single narrow column (420×700), `Full` the wide two-column layout (880×760),
  `Custom` tracks the last manually-dragged size. Persisted (incl. the custom
  width/height).
- `HistoryBase { All, Hex, Dec, Bin, Oct }` — which base(s) history rows show;
  persisted.
- `HistoryResult { Integer { value, sign }, Float(f64) }` — a result in the mode
  that produced it. `HistoryEntry { expr, result }` pairs the typed expression
  with its result, so recall restores the right mode and a faithful decimal.

### Core state-update methods

| Method | Responsibility |
|--------|----------------|
| `refresh(skip)` | Rewrite every text buffer from the live value, except the `skip` field. Mode-aware: in float mode `dec` shows the `f64` and hex/bin/oct show its IEEE-754 pattern. |
| `format_fixed()` | Render the value as a fixed-point real (via `core::fixed`). |
| `copy(ctx, text, label)` | Copy to clipboard and flash a short auto-dismissing toast. |
| `error(msg)` | Show a persistent error toast (cleared on the next success). |
| `set_width(bits)` | Clamp width, re-mask value, clamp `frac_bits`, refresh. |
| `set_sign(sign)` | Change signedness, refresh (decimal re-renders). |
| `is_float_mode()` | The single source of truth for whether float mode is active. |
| `set_number_mode(mode)` | Switch int/float, seeding the destination value so `ans` carries over; refresh. |
| `on_field_edit(field)` | Parse a base buffer → new `value`, refresh others (integer mode). |
| `on_field_edit_float(field)` | Float mode: `dec` accepts a real; hex/bin/oct reinterpret an entered 64-bit pattern as an `f64`. |
| `on_fixed_edit()` | Parse the real-number buffer → `value` via `fixed::from_real`. |
| `eval_expr()` | Evaluate `expr` via `core::eval` (integer) or `core::eval_float` (float), push to history, refresh; set `expr_error` on failure. |
| `push_history(expr, result)` | Append a `HistoryEntry`, trimming to `MAX_HISTORY = 200`. |
| `recall(entry)` | Restore a history entry's mode, value/width/sign (or float), and expr text. |

### Layout (`App::ui`)

Each frame: apply the theme; handle a one-shot startup resize and detect manual
window resizes (using `content_rect`, which — unlike `inner_rect` — is reliable
under WSL/glow); then a `CentralPanel` → vertical `ScrollArea` → header → body.

- **Header:** title "nybble" (+ a debug-only window-size readout), a **View**
  button cycling Compact → Full → Custom, and a **Theme** button cycling
  Auto → Light → Dark.
- **Body:** the expression centerpiece spans full width on top. Below it, a
  **two-column** layout when there's room (`view_mode != Compact` and available
  width ≥ 720), collapsing to a single stack otherwise. Order: Current value →
  Bits → Format → History.
- In **float mode** the bit grid and the integer-only Format controls (width,
  sign, fixed-point) are hidden, since the value is an `f64` rather than
  width-bound bits.

A `status` toast renders at the bottom when set. Sections are wrapped in rounded
filled "cards" by the `App::section` helper. `PC_DEBUG=1` dumps the layout
decision and bit-grid row geometry to stderr for layout debugging.

### Persistence

`App::new` reads `theme_mode`, `history_base`, `view_mode`, `number_mode`, and the
custom window size (`custom_w`/`custom_h`) from eframe storage (falling back to
defaults); `App::save` writes them back. The value and expression history are
session-only.

### Theming (`theme.rs`)

`ThemeMode { Auto, Light, Dark }` (persisted; `Auto` follows the OS, defaulting to
Dark when unknown). `apply(ctx, mode)` resolves the mode to an `egui::Theme` and
installs a `Style` built by `build_style` (rounded widgets, accent strokes,
tuned spacing/typography, and `selectable_labels = false` for a consistent button
feel). A `Palette` struct defines a cohesive indigo-accented dark/light color set.
The module exposes helpers `card_fill`, `accent`, `on_accent`. Note in code:
`widgets.active.fg_stroke` doubles as egui's app-wide "strong" text color, so it is
kept legible and the accent is applied explicitly where needed.

### Widgets (`widgets/`)

`widgets/mod.rs` re-exports `bit_grid`. `widgets/bitgrid.rs::bit_grid(ui, value,
accent) -> Option<Value>` renders the value's bits **MSB→LSB** as: a header
(highest bit index · set count · `bit 0`), a thin clickable **bit bar** (one
segment per bit, gap on nibble boundaries), and a grid of clickable 0/1 cells
packed into non-breaking **nibble groups**, with as many whole groups per row as
fit the available width. Each nibble group carries an index marker (its lowest bit
number) underneath, Windows-calculator style. Clicking a bit (bar or grid) returns
the value with that bit toggled, else `None`. Private helpers: `bit_bar`,
`contrast_text` (luminance-based black/white over the accent fill).

## Free helper functions (`app.rs`)

`field_label`, `paint_input_frame` (accent outline + left-edge stripe marking an
editable field), `section_label`, `mini_value_line` (click-to-copy small base
line), `value_lines` / `float_value_lines` / `value_line` (history result
rendering, click-to-copy), and `parse_base(text, radix, width, sign)` /
`strip_radix_prefix` — the GUI's own base-field parser (strips whitespace/`_`,
accepts optional `0x`/`0b`/`0o` prefixes, and a leading `-` only in signed decimal
mode). Empty input parses to zero.

## Testing strategy

The GUI crate has **no automated tests** — by design, all testable logic lives in
`nybble-core`. GUI verification is manual (`cargo run -p nybble-gui`); see
the verification steps in [`nybble-plan.md`](./nybble-plan.md) and the
[Development Guide](./development-guide.md).

## Component inventory

See [Component Inventory — GUI](./component-inventory-gui.md) for the catalog of
UI sections and widgets. Core integration details are in
[Integration Architecture](./integration-architecture.md).
</content>
