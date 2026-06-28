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
- **`changelog.rs`** — embedded `ENTRIES` release notes powering the "What's new" dialog. `App` auto-opens it once when the running version differs from the persisted `last_seen_version`; the header version label reopens it.

eframe is pulled with `default-features = false`; only `glow`, `default_fonts`, `persistence`, and `x11` (unix only) are enabled. `wayland` was dropped (crashes under WSL); `accesskit` disabled (needs D-Bus).

## Data flow

User input → mutate `Value` via core → `App::refresh()` rewrites text buffers → next frame redraws everything.

In float mode, `float_value: f64` is the currency instead of `Value`.

## Persisted preferences

`theme_mode`, `history_base`, `view_mode`, `number_mode`, `custom_w`/`custom_h`, `auto_check_updates`, `last_seen_version` (drives the "What's new" dialog — shown once when the running version differs), and the settings keys (`panel_order`, `panel_*`, `field_*`, `show_fixed_point`/`show_bit_slicer`, `copy_*`) — stored via eframe's persistence feature. Value and history are session-only.

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
| Add release notes / changelog entry | `crates/gui/src/changelog.rs` (`ENTRIES`, newest first) + bump `version` in `crates/gui/Cargo.toml` to match |
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

## Releasing

To cut version `X.Y.Z`, produce a single **release commit** (the maintainer creates and pushes the git tag + GitHub release afterwards). The version lives in exactly three committed places — both crate manifests and the lockfile — plus the embedded changelog. Both crates are kept in lockstep; the app displays the `nybble-gui` version (`env!("CARGO_PKG_VERSION")`).

1. **Bump the version** to `X.Y.Z` in:
   - `crates/core/Cargo.toml` (the `version =` line)
   - `crates/gui/Cargo.toml` (the `version =` line)
2. **Add a changelog entry** at the top of `ENTRIES` in `crates/gui/src/changelog.rs`, newest first:
   ```rust
   ReleaseNotes {
       version: "X.Y.Z", // must match the Cargo.toml version exactly
       items: &[
           "User-facing summary of each notable change.",
       ],
   },
   ```
   Write `items` from the user's perspective (what's new/changed/fixed). The `version` string **must equal** the manifest version, or `notes_for(current)` returns `None` and the post-update "What's new" dialog shows nothing.
3. **Refresh the lockfile and verify**: `cargo build` (rewrites the two `nybble-*` entries in the committed `Cargo.lock`) then `cargo test` — both must be green. Optionally `cargo run -p nybble-gui` and click the header version label to confirm the new notes render.
4. **Commit atomically** — all four files in one commit, nothing else:
   ```sh
   git add crates/core/Cargo.toml crates/gui/Cargo.toml Cargo.lock crates/gui/src/changelog.rs
   git commit -m "Release vX.Y.Z"
   ```

Left to the maintainer (do **not** do these automatically): `git tag vX.Y.Z`, push, and publish the matching GitHub release on `fanaloka47/nybble` — that release is what the in-app updater (`self_update`, `crates/gui/src/update.rs`) polls to offer the upgrade.

## Constraints

- Values capped at **128 bits**. Wider support needs an arbitrary-precision backend (future extension).
- Fixed-point conversion goes through `f64` — display only, raw bits stay exact.
- `Cargo.lock` is committed intentionally for reproducible builds.
- No CI configured. Distribution is a single self-contained binary per OS.
