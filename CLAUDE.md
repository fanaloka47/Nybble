# Nybble — CLAUDE.md

## What this is

Nybble is a native desktop calculator for FPGA/hardware engineers. It shows one value in hex, decimal, binary, and octal simultaneously, supports bit-width and signedness, lets users flip individual bits, and evaluates bitwise/arithmetic expressions — including the `**` power operator and named functions (`sqrt`, `log2`, `clog2`, `gcd`, … in integer mode; the full scientific set plus `pi`/`e`/`tau` in float mode). Targets Windows and Linux.

## Workspace layout

Cargo workspace (resolver 2) with two crates:

| Crate | Path | Role |
|-------|------|------|
| `nybble-core` | `crates/core` | Pure numeric library, no deps, all tests live here |
| `nybble-gui` | `crates/gui` | eframe/egui 0.34 desktop app, binary named `nybble` |

**Core design rule:** all numeric logic lives in `nybble-core`. The GUI is a thin presentation layer. Never put numeric decisions in the GUI.

## Common commands

```sh
cargo run -p nybble-gui          # run the app
cargo test                       # run all 58 unit tests (core only)
cargo test -p nybble-core        # core tests only
cargo build --release            # → target/release/nybble
cargo fmt && cargo clippy
```

## Debug knobs

| Knob | Effect |
|------|--------|
| `PC_SIZE=WIDTHxHEIGHT` | Override initial window size (reproduce layout bugs) |
| `PC_DEBUG=1` | Dump layout decision + bit-grid geometry to stderr |
| `--features screenshot` + `EFRAME_SCREENSHOT_TO=/path.png` | Save screenshot and exit |

## Architecture — core (`crates/core/src/`)

- **`value.rs`** — `Value { raw: u128, width: Width }`, always masked. `Width(u32)` is 1–128. `Signedness` is not stored on a value — passed in at render/shift/div time.
- **`ops.rs`** — all arithmetic/bitwise methods on `Value`. Results are re-masked to the left operand's width (hardware truncation semantics).
- **`expr.rs`** — tokenizer + Pratt parser + evaluator for integer expressions. `ans` identifier refers to the current value. `eval(input, width, sign, ans) -> Result<Value, EvalError>`.
- **`float.rs`** — parallel `f64` evaluator for float mode. `eval_float(input, ans: f64) -> Result<f64, EvalError>`. Rejects bitwise/shift operators.
- **`parse.rs`** — `parse_literal` for single tokens (`0x`, `0b`, `0o` prefixes; `_` separators allowed).
- **`fixed.rs`** — Qm.n fixed-point conversion via `f64`. Display convenience only; raw bits stay exact.

Public API re-exported from `lib.rs`: `Value`, `Width`, `Signedness`, `eval`, `EvalError`, `eval_float`, `f64_to_value`, `parse_literal`, `ParseError`.

## Architecture — GUI (`crates/gui/src/`)

- **`main.rs`** — eframe entry point, 760×720 default window, 520×480 minimum, Glow (OpenGL) backend.
- **`app.rs`** — `struct App` holds all state. Immediate-mode: `ui()` runs every frame. Key state: `value: Value`, `width`, `sign`, `frac_bits`, `float_value: f64`, `number_mode: NumberMode`, one text buffer per base.
- **`settings.rs`** — `Settings` (panel order/visibility, per-field toggles, `CopyOptions`) edited via the gear-icon modal. `Panel` enum drives the data-driven layout in `App::ui`; `CopyOptions::apply` is the single clipboard transform. Self-contained string KV (de)serialization plumbed through `App::new`/`App::save`.
- **`theme.rs`** — `ThemeMode { Auto, Light, Dark }`, indigo-accented palette. `widgets.active.fg_stroke` doubles as egui's "strong" text color.
- **`widgets/bitgrid.rs`** — clickable bit grid MSB→LSB, nibble-grouped, returns toggled `Value` on click.

eframe is pulled with `default-features = false`; only `glow`, `default_fonts`, `persistence`, and `x11` (unix only) are enabled. `wayland` was dropped (crashes under WSL); `accesskit` disabled (needs D-Bus).

## Data flow

User input → mutate `Value` via core → `App::refresh()` rewrites text buffers → next frame redraws everything.

In float mode, `float_value: f64` is the currency instead of `Value`.

## Persisted preferences

`theme_mode`, `history_base`, `view_mode`, `number_mode`, `custom_w`/`custom_h`, `auto_check_updates`, and the settings keys (`panel_order`, `panel_*`, `field_*`, `show_fixed_point`/`show_bit_slicer`, `copy_*`) — stored via eframe's persistence feature. Value and history are session-only.

## Where to make changes

| Task | File |
|------|------|
| Add a numeric operation | `crates/core/src/ops.rs` (+ tests) |
| Add an expression operator | `crates/core/src/expr.rs` (`Token`, `tokenize`, `infix_bp`, `apply_infix`) |
| Add a named integer function | `crates/core/src/ops.rs` (numeric method) + wire into `Parser::dispatch` in `expr.rs` |
| Add a float-mode function/constant | `crates/core/src/float.rs` (`call_func` / `constant`) |
| Add/adjust a base format | `crates/core/src/value.rs` (`to_hex`/`to_bin`/`to_oct`/`to_dec`) |
| Add a float-mode operator | `crates/core/src/float.rs` |
| Add a UI section | `crates/gui/src/app.rs` (new fn on `App`, add a `Panel` variant in `settings.rs`, wire into `App::render_panel`) |
| Add/adjust a setting | `crates/gui/src/settings.rs` (struct field + load/save) and the modal in `App::settings_body` |
| Add a reusable widget | `crates/gui/src/widgets/` (new module + re-export in `mod.rs`) |
| Tweak colors/spacing | `crates/gui/src/theme.rs` |

## Testing

Most tests are in `nybble-core` (58); `nybble-gui` has a handful in `settings.rs` covering copy transforms and reordering. The rest of the GUI has no automated tests — verify manually with `cargo run -p nybble-gui`.

Canonical smoke test:
1. `cargo test` — all green.
2. Type `DEAD_BEEF` in HEX → DEC shows `3735928559`, BIN/OCT update live.
3. Toggle bits and watch all bases update.
4. Switch 8↔32 and signed↔unsigned.
5. Evaluate `0xFF & (1 << 3)`.
6. Evaluate `clog2(1024)` → `10` and `2**8` → `256` (integer mode); switch to float mode and evaluate `sqrt(2)` and `sin(pi/2)`.
7. Open the gear-icon Settings: disable OCT (field disappears), reorder a panel, toggle a copy option (preview updates), then restart to confirm it persisted.

## Git workflow

Always make atomic commits — one logical change per commit. Never bundle unrelated changes into a single commit.

## Constraints

- Values capped at **128 bits**. Wider support needs an arbitrary-precision backend (future extension).
- Fixed-point conversion goes through `f64` — display only, raw bits stay exact.
- `Cargo.lock` is committed intentionally for reproducible builds.
- No CI configured. Distribution is a single self-contained binary per OS.
