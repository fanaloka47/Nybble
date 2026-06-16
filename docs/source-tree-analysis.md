# Source Tree Analysis

_Generated: 2026-06-16 · Scan level: exhaustive_

## Annotated tree

```
powercalc/
├── Cargo.toml                  # Workspace manifest (resolver 2; members: core, gui)
├── Cargo.lock                  # Committed (reproducible builds for the binary)
├── README.md                   # User-facing intro, features, build & run
├── cdc.txt                     # Original product brief from the FPGA engineer
├── docs/                       # ← Generated AI-context documentation (this folder)
│   └── powercalc-plan.md       # Step-by-step build plan / design decisions
└── crates/
    ├── core/                   # Part: core — powercalc-core (library, no UI deps)
    │   ├── Cargo.toml          # lib crate: powercalc_core
    │   └── src/
    │       ├── lib.rs          # Crate root; module decls + public re-exports
    │       ├── value.rs        # Width, Signedness, Value (canonical model) + formatters
    │       ├── ops.rs          # Width-correct bitwise/arith/shift/rotate ops on Value
    │       ├── parse.rs        # Single-literal parser (0x/0b/0o/dec, `_` separators)
    │       ├── expr.rs         # Tokenizer + Pratt parser + evaluator (eval)
    │       └── fixed.rs        # Fixed-point Qm.n <-> real conversion
    └── gui/                    # Part: gui — powercalc-gui (binary: `powercalc`)
        ├── Cargo.toml          # bin crate; depends on powercalc-core + eframe/egui
        └── src/
            ├── main.rs         # Entry point: NativeOptions + eframe::run_native  [ENTRY]
            ├── app.rs          # App state + all UI sections (eframe::App impl)
            ├── theme.rs        # Dark/Light/Auto palettes, egui Style builder
            └── widgets/
                ├── mod.rs      # Re-exports bit_grid
                └── bitgrid.rs  # Interactive MSB→LSB clickable bit grid
```

## Critical directories

| Path                    | Purpose |
|-------------------------|---------|
| `crates/core/src/`      | All numeric logic. Pure functions/methods on `Value`; every file ends with a `#[cfg(test)]` module. This is where correctness lives. |
| `crates/gui/src/`       | The desktop app. State + rendering only — delegates all math to core. |
| `crates/gui/src/widgets/` | Reusable egui widgets (currently just the bit grid). |
| `docs/`                 | Generated documentation (planning + this scan output). |

## Entry points

- **Binary entry:** `crates/gui/src/main.rs` → `main()` builds `eframe::NativeOptions`
  (Glow renderer, 760×720 default window) and calls `eframe::run_native`, handing
  off to `app::App`.
- **Library root:** `crates/core/src/lib.rs` re-exports the public API
  (`Value`, `Width`, `Signedness`, `eval`, `EvalError`, `parse_literal`,
  `ParseError`) plus the `expr`, `fixed`, `ops`, `parse`, `value` modules.

## Per-file size (lines)

| File | LOC |
|------|-----|
| `crates/gui/src/app.rs`            | 726 |
| `crates/core/src/expr.rs`          | 339 |
| `crates/core/src/value.rs`         | 259 |
| `crates/core/src/ops.rs`           | 249 |
| `crates/gui/src/theme.rs`          | 224 |
| `crates/core/src/parse.rs`         | 113 |
| `crates/core/src/fixed.rs`         | 96  |
| `crates/gui/src/widgets/bitgrid.rs`| 78  |
| `crates/gui/src/main.rs`           | 22  |
| `crates/core/src/lib.rs`           | 17  |
| `crates/gui/src/widgets/mod.rs`    | 5   |
| **Total**                          | **~2,128** |

## How the parts interface

`powercalc-gui` depends on `powercalc-core` via a path dependency
(`powercalc-core = { path = "../core" }`). The GUI holds a single `Value` plus
UI-only state (text buffers, history, theme) and calls into core for every
numeric transformation. See [Integration Architecture](./integration-architecture.md).
