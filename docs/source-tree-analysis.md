# Source Tree Analysis

_Generated: 2026-06-16 · Updated: 2026-06-23 (full rescan) · Scan level: exhaustive_

## Annotated tree

```
powercalc/
├── Cargo.toml                  # Workspace manifest (resolver 2; members: core, gui)
├── Cargo.lock                  # Committed (reproducible builds for the binary)
├── README.md                   # User-facing intro, features, build & run
├── cdc.txt                     # Original product brief from the FPGA engineer
├── docs/                       # ← Generated AI-context documentation (this folder)
│   └── powercalc-plan.md       # Step-by-step build plan / design decisions
├── resources/                  # Claude Design GUI-modernization handoff (HTML/JS mockups)
└── crates/
    ├── core/                   # Part: core — powercalc-core (library, no UI deps)
    │   ├── Cargo.toml          # lib crate: powercalc_core
    │   └── src/
    │       ├── lib.rs          # Crate root; module decls + public re-exports
    │       ├── value.rs        # Width, Signedness, Value (canonical model) + formatters
    │       ├── ops.rs          # Width-correct bitwise/arith/shift/rotate ops on Value
    │       ├── parse.rs        # Single-literal parser (0x/0b/0o/dec, `_` separators)
    │       ├── expr.rs         # Integer tokenizer + Pratt parser + evaluator (eval)
    │       ├── float.rs        # Full-precision f64 evaluator (eval_float, f64_to_value)
    │       └── fixed.rs        # Fixed-point Qm.n <-> real conversion
    └── gui/                    # Part: gui — powercalc-gui (binary: `powercalc`)
        ├── Cargo.toml          # bin crate; depends on powercalc-core + eframe/egui
        └── src/
            ├── main.rs         # Entry point: NativeOptions + eframe::run_native  [ENTRY]
            ├── app.rs          # App state + all UI sections (eframe::App impl)
            ├── theme.rs        # Dark/Light/Auto palettes, egui Style builder
            └── widgets/
                ├── mod.rs      # Re-exports bit_grid
                └── bitgrid.rs  # Interactive MSB→LSB bit grid (bar + clickable cells + markers)
```

## Critical directories

| Path                    | Purpose |
|-------------------------|---------|
| `crates/core/src/`      | All numeric logic. Pure functions/methods on `Value`; every file ends with a `#[cfg(test)]` module. This is where correctness lives. |
| `crates/gui/src/`       | The desktop app. State + rendering only — delegates all math to core. |
| `crates/gui/src/widgets/` | Reusable egui widgets (currently just the bit grid). |
| `docs/`                 | Generated documentation (planning + this scan output). |
| `resources/`            | Design-handoff artifacts (the Claude Design GUI mockup); not compiled. |

## Entry points

- **Binary entry:** `crates/gui/src/main.rs` → `main()` builds `eframe::NativeOptions`
  (Glow renderer, 760×720 default window, honouring a `PC_SIZE=WxH` override) and
  calls `eframe::run_native`, handing off to `app::App`.
- **Library root:** `crates/core/src/lib.rs` re-exports the public API
  (`Value`, `Width`, `Signedness`, `eval`, `EvalError`, `eval_float`,
  `f64_to_value`, `parse_literal`, `ParseError`) plus the `expr`, `fixed`, `float`,
  `ops`, `parse`, `value` modules.

## Per-file size (lines)

| File | LOC |
|------|-----|
| `crates/gui/src/app.rs`            | 1396 |
| `crates/core/src/expr.rs`          | 344 |
| `crates/core/src/float.rs`         | 270 |
| `crates/core/src/value.rs`         | 259 |
| `crates/core/src/ops.rs`           | 249 |
| `crates/gui/src/theme.rs`          | 227 |
| `crates/gui/src/widgets/bitgrid.rs`| 189 |
| `crates/core/src/parse.rs`         | 113 |
| `crates/core/src/fixed.rs`         | 96  |
| `crates/gui/src/main.rs`           | 36  |
| `crates/core/src/lib.rs`           | 19  |
| `crates/gui/src/widgets/mod.rs`    | 5   |
| **Total**                          | **~3,203** |

## How the parts interface

`powercalc-gui` depends on `powercalc-core` via a path dependency
(`powercalc-core = { path = "../core" }`). The GUI holds a single `Value` (plus a
parallel `f64` for float mode) and UI-only state (text buffers, history, theme,
view mode) and calls into core for every numeric transformation. See
[Integration Architecture](./integration-architecture.md).
</content>
</invoke>
