# PowerCalc — Multi-Base Calculator for FPGA Work

## Context

An FPGA engineer constantly converts and computes across hexadecimal, decimal, and binary,
and can't find a calculator that makes this fluid. The goal is a **fast, easy-to-use native
desktop app** (Windows + Linux) whose primary purpose is *playing with number
representations* — see one value in every base at once, flip bits, choose widths/signedness,
and run bitwise/arithmetic expressions.

Decisions locked in:
- **Form factor:** native desktop GUI
- **Language:** Rust (new to Rust, want to learn it)
- **Features:** bitwise ops, width & signedness, interactive bit grid, expressions + fixed-point
- **Interaction:** live multi-base fields (edit any base → all others update), plus expression line
- **Stack:** `eframe` + `egui` 0.34 — pure Rust (no HTML/JS to learn too), single self-contained
  binary per OS, immediate-mode rendering that fits live fields + clickable bit grid. Most
  beginner-friendly Rust GUI path while staying fast and shippable.

Greenfield project — directory holds only BMAD scaffolding, no code yet.

### Target structure (built up across the steps below)

```
powercalc/
  Cargo.toml                  # workspace
  crates/
    core/                     # powercalc-core (library, no UI deps)
      src/{lib,value,ops,parse,expr,fixed}.rs
      tests/
    gui/                      # powercalc-gui (binary → renamed `powercalc`)
      src/main.rs
      src/app.rs
      src/widgets/{bitgrid,basefield}.rs
```

Design rule throughout: **all numeric logic lives in `core` and is unit-tested without the
GUI.** The canonical value is a `u128` raw bit pattern, always masked to the current width
(1..=128 bits). Signedness only affects decimal display (two's complement) and arithmetic vs
logical right shift; hex/bin/oct always show the raw masked pattern. (>128-bit buses are out
of scope for v1 — note BigInt as a future extension.)

---

## Step 1 — Workspace + core value model

**Goal:** project scaffolding plus the canonical value, width, signedness, masking, and
base formatting.

- Create workspace `Cargo.toml` and `crates/core` (lib) + `crates/gui` (bin) skeletons.
- `core/src/value.rs`: `Width(1..=128)`, `Signedness {Unsigned, Signed}`, `Value { raw: u128 }`
  always masked to width. Formatters to hex / dec / bin / oct with digit grouping
  (`DEAD_BEEF`, `1101_1110`); decimal honors signedness (two's complement).
- `core/src/lib.rs`: re-export the public API.

**Verify:** `cargo test -p powercalc-core` — masking, two's-complement decimal round-trips,
grouping, signed vs unsigned decimal for the same bit pattern (e.g. 0xFF = 255 unsigned /
-1 signed at width 8).

**Done when:** core compiles and value/formatting tests pass.

---

## Step 2 — Operations

**Goal:** the FPGA operation set, width-correct.

- `core/src/ops.rs`: `AND OR XOR NOT NAND NOR XNOR`; `<<`, `>>` (logical, arithmetic when
  signed); rotate-left / rotate-right within width; `+ - * / %`. Every result re-masked to
  width (hardware-style truncation/overflow).

**Verify:** `cargo test -p powercalc-core` — arithmetic vs logical shift, rotates wrap within
width, overflow truncates, NOT respects width.

**Done when:** all op tests pass.

---

## Step 3 — Literal parsing + expression evaluator

**Goal:** type expressions mixing bases and compute a new value.

- `core/src/parse.rs`: parse literals `0xFF`, `0b1010`, `0o17`, `255`, with optional `_`
  separators.
- `core/src/expr.rs`: hand-rolled tokenizer + Pratt parser (dependency-free; good Rust
  learning artifact). Full operator set from Step 2, parentheses, unary `~`/`-`, and `ans`
  (current value). Evaluation respects current width/signedness.

**Verify:** `cargo test -p powercalc-core` — operator precedence, mixed-base literals,
parentheses, `0xFF & (1<<3)`, `ans` substitution, malformed input returns a clean error.

**Done when:** evaluator tests pass.

---

## Step 4 — Fixed-point (Qm.n)

**Goal:** DSP-style fixed-point view.

- `core/src/fixed.rs`: interpret raw integer as `Qm.n` (`real = raw / 2^n`) for display, and
  convert an entered real back to the integer; `n` user-selectable.

**Verify:** `cargo test -p powercalc-core` — round-trips for representative Qm.n values, signed
fixed-point.

**Done when:** fixed-point tests pass. **Core is now feature-complete and fully tested.**

---

## Step 5 — GUI skeleton + live base fields

**Goal:** a window where editing any base updates all the others.

- Add `eframe`/`egui` 0.34 to `crates/gui`. `gui/src/main.rs` opens the window;
  `gui/src/app.rs` holds the `App` state (`Value`, `Width`, `Signedness`, per-field text
  buffers, error flag).
- `gui/src/widgets/basefield.rs`: one editable row (HEX/DEC/BIN/OCT). Editing reparses into
  the canonical `Value` via `core`; all rows re-render the same frame. Invalid input shows an
  inline error tint without clearing the field.

**Verify:** `cargo run -p powercalc-gui` — type `DEAD_BEEF` in HEX → DEC shows `3735928559`,
BIN/OCT update live; bad input tints red and doesn't crash.

**Done when:** live base fields work end-to-end against core.

---

## Step 6 — Interactive bit grid

**Goal:** the core "play" interaction — flip bits, watch every base change.

- `gui/src/widgets/bitgrid.rs`: width-many toggle buttons, MSB→LSB, grouped in rows of 8/16
  with bit-index labels. Clicking a bit toggles it in the canonical `Value`.

**Verify:** `cargo run -p powercalc-gui` — toggling bits updates all base fields instantly;
grid resizes with the width control (Step 8).

**Done when:** bit grid is bidirectional with the live fields.

---

## Step 7 — Expression line + operator buttons

**Goal:** computation path on top of the live view.

- Expression text line: press Enter → result (via `core::expr`) becomes the current value,
  refreshing fields + grid.
- Bitwise/arith buttons (`AND OR XOR << >> ~` …) append the matching token to the expression
  line — one coherent compute path rather than a separate register calculator.

**Verify:** `cargo run -p powercalc-gui` — typing `0xFF & (1<<3)` + Enter renders the result in
all bases; op buttons build expressions correctly.

**Done when:** expressions and op buttons drive the shared value.

---

## Step 8 — Controls bar (width / signedness / fixed-point)

**Goal:** reinterpret the same bits under different formats.

- Width presets 8/16/32/64 + custom slider; signed/unsigned toggle; optional Qm.n controls
  wired to `core::fixed`.

**Verify:** `cargo run -p powercalc-gui` — switching 8↔32 and signed↔unsigned reinterprets
decimal correctly and resizes the bit grid; fixed-point view updates with `n`.

**Done when:** all controls reinterpret the live value correctly.

---

## Step 9 — Polish + release builds

**Goal:** ship it.

- Digit grouping everywhere, consistent error states, sensible keyboard focus/Tab order,
  rename binary to `powercalc`.
- `cargo build --release` on Linux (and Windows when available) → single self-contained binary.
- (Optional, later) GitHub Actions matrix to emit Windows `.exe` + Linux binary on tag.

**Verify:** `cargo build --release` succeeds; launched release binary passes the Step 5–8 checks.

**Done when:** release binary runs standalone on Linux.

---

## Final end-to-end verification

1. `cargo test` — entire core test suite green.
2. `cargo run -p powercalc-gui` — HEX `DEAD_BEEF` → DEC `3735928559` + BIN/OCT; bit toggle
   updates all bases; 8↔32 and signed↔unsigned reinterpret decimal; `0xFF & (1<<3)` evaluates
   in all bases.
3. `cargo build --release` — standalone binary on Linux (Windows when available).

## References

- [eframe on crates.io](https://crates.io/crates/eframe)
- [egui on GitHub](https://github.com/emilk/egui)
