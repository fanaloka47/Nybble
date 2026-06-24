# Development Guide

_Generated: 2026-06-16 · Updated: 2026-06-23 (full rescan) · Scan level: exhaustive_

## Prerequisites

- **Rust toolchain** (edition 2021) — install via [rustup](https://rustup.rs).
  Cargo comes with it. No other system packages are required to build the core
  library.
- **For the GUI on Linux:** an OpenGL-capable environment. The app uses the
  `glow` (OpenGL) backend with the **X11** windowing feature; it is known to run
  under **WSLg**. The `wayland` feature was dropped because it crashed on launch
  under WSL. `accesskit` is intentionally disabled (it needs a D-Bus session that
  WSL often lacks), so no D-Bus is required.

## Workspace layout

A Cargo workspace (resolver 2) with two members:

| Crate | Path | Kind | Output |
|-------|------|------|--------|
| `nybble-core` | `crates/core` | library (`nybble_core`) | — |
| `nybble-gui`  | `crates/gui`  | binary | `nybble` |

See [Source Tree Analysis](./source-tree-analysis.md) for the annotated tree.

## Common commands

```sh
# Run the desktop app
cargo run -p nybble-gui

# Run the core test suite (43 unit tests; the GUI has none)
cargo test                       # whole workspace
cargo test -p nybble-core     # core only

# Build a release binary → target/release/nybble
cargo build --release

# Lint / format (standard Cargo tooling)
cargo fmt
cargo clippy
```

### Debug / environment knobs

| Var / flag | Effect |
|------------|--------|
| `PC_SIZE=WIDTHxHEIGHT` | Override the initial window size (reproduce a layout bug at an exact size). |
| `PC_DEBUG=1` | Dump the layout decision + bit-grid row geometry to stderr; also shows a window-size readout in the header (debug builds). |
| `--features screenshot` + `EFRAME_SCREENSHOT_TO=/path.png` | Save a screenshot and exit (uses eframe's `__screenshot`). |

## Environment & configuration

- **No `.env` files, no runtime config files.** Behavior is controlled entirely
  through the UI (and the debug env knobs above).
- **Persisted preferences:** the GUI saves `theme_mode`, `history_base`,
  `view_mode`, `number_mode`, and the custom window size (`custom_w`/`custom_h`)
  via eframe's `persistence` feature (platform-standard app-storage location
  managed by eframe). The value and expression history are **not** persisted
  across runs.

## Testing approach

- All testable logic lives in `nybble-core`; each module has a
  `#[cfg(test)] mod tests` block. Coverage: `value.rs` (10), `expr.rs` (8),
  `float.rs` (8), `ops.rs` (7), `fixed.rs` (6), `parse.rs` (4) — **43 total**.
- Tests assert hardware-style semantics directly: width masking/truncation,
  two's-complement round-trips, signed vs unsigned decimal, logical vs arithmetic
  shift, rotate wraparound, operator precedence, mixed-base literals, `ans`
  substitution, error cases, fixed-point round-trips, and float-mode behaviour
  (scientific notation, integer widening, IEEE inf/nan, bitwise rejection).
- The GUI is verified **manually**. Canonical smoke test (from
  [`nybble-plan.md`](./nybble-plan.md)):
  1. `cargo test` — entire core suite green.
  2. `cargo run -p nybble-gui` — type `DEAD_BEEF` in HEX → DEC shows
     `3735928559`, BIN/OCT update live; toggle bits and watch all bases update;
     switch 8↔32 and signed↔unsigned to reinterpret decimal; evaluate
     `0xFF & (1 << 3)` → result in all bases.
  3. `cargo build --release` — standalone binary on Linux (Windows when available).

## Common development tasks

| Task | Where |
|------|-------|
| Add a numeric operation | `crates/core/src/ops.rs` (+ tests); expose via the expression evaluator in `crates/core/src/expr.rs` if it should be typeable. |
| Add an expression operator/precedence change | `crates/core/src/expr.rs` (`Token`, `tokenize`, `infix_bp`, `apply_infix`). |
| Add/adjust a base format | `crates/core/src/value.rs` (`to_hex`/`to_bin`/`to_oct`/`to_dec`, `group`). |
| Add a float-mode operator/behaviour | `crates/core/src/float.rs` (`tokenize`, `infix_bp`, `Parser`, + tests). |
| Add a UI section | `crates/gui/src/app.rs` (new `fn` on `App`, wire into `App::ui`). |
| Change how float mode is triggered | `crates/gui/src/app.rs` (`App::is_float_mode` / `set_number_mode` — the single chokepoint). |
| Add a reusable widget | `crates/gui/src/widgets/` (new module + re-export in `mod.rs`). |
| Tweak colors/spacing/typography | `crates/gui/src/theme.rs` (`palette`, `build_style`). |

## Build notes & constraints

- `Cargo.lock` is **committed intentionally** for reproducible binary builds (see
  the comment in `.gitignore`).
- eframe is pulled with `default-features = false` and an explicit feature set
  (`glow`, `default_fonts`, `persistence`, plus `x11` only under `cfg(unix)`) —
  keep changes to these deliberate (the choice trades the wgpu/Vulkan backend,
  accesskit, and wayland for WSL reliability). An optional `screenshot` feature
  gates eframe's `__screenshot` support.
- Values are capped at **128 bits**; wider support would require an
  arbitrary-precision backend (future extension).

## CI / deployment

No CI is configured in the repository today. The plan notes an optional future
GitHub Actions matrix to emit a Windows `.exe` + Linux binary on tag. Distribution
is a single self-contained binary per OS produced by `cargo build --release`.
