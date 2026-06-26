# nybble — Documentation Index

_Generated: 2026-06-16 · Updated: 2026-06-23 (full rescan) · Scan level: exhaustive · Primary AI-context entry point_

nybble is a fast native desktop calculator for working across number bases
(hex/dec/bin/oct) with bit-width, signedness, an interactive bit grid, an integer
and a full-precision float expression evaluator, and fixed-point — built for
FPGA/hardware work.

## Project overview

- **Type:** monorepo (Rust Cargo workspace, 2 parts)
- **Primary language:** Rust (edition 2021)
- **Architecture:** layered — pure numeric core + thin eframe/egui desktop UI
- **Binary:** `nybble` (from `nybble-gui`)
- **Tests:** 43 unit tests, all in `nybble-core`

## Quick reference by part

### core — `nybble-core` (library)
- **Root:** `crates/core`
- **Tech:** Rust, no dependencies (std only)
- **Entry/API:** `crates/core/src/lib.rs` re-exports `Value`, `Width`,
  `Signedness`, `eval`, `EvalError`, `eval_float`, `f64_to_value`,
  `parse_literal`, `ParseError`
- **Holds:** value model, width-correct ops, literal parsing, an integer
  expression evaluator (tokenizer + Pratt parser), a full-precision `f64`
  evaluator (float mode), fixed-point — and the whole test suite

### gui — `nybble-gui` (desktop)
- **Root:** `crates/gui`
- **Tech:** `eframe`/`egui` 0.34, Glow (OpenGL) backend, x11 (wayland dropped)
- **Entry point:** `crates/gui/src/main.rs` → `eframe::run_native` → `app::App`
- **Holds:** app state + immediate-mode UI (int/float modes, view modes, theming);
  delegates all math to core

## Generated documentation

- [Project Overview](./project-overview.md)
- [Architecture — Core](./architecture-core.md)
- [Architecture — GUI](./architecture-gui.md)
- [Source Tree Analysis](./source-tree-analysis.md)
- [API Contracts — Core](./api-contracts-core.md)
- [Component Inventory — GUI](./component-inventory-gui.md)
- [Integration Architecture (core ↔ gui)](./integration-architecture.md)
- [Development Guide](./development-guide.md)
- [Supported expressions](./expressions.md) — complete operator, function & constant reference
- [Project Parts metadata](./project-parts.json)

## Existing documentation

- [README](../README.md) — user-facing intro, features, build & run
- [nybble Plan](./nybble-plan.md) — step-by-step build plan & design decisions
- [Product brief (`cdc.txt`)](../cdc.txt) — original request from the FPGA engineer

## Getting started

```sh
cargo run -p nybble-gui      # launch the app
cargo test                      # run the core test suite (43 tests)
cargo build --release           # produce target/release/nybble
```

Requires a Rust toolchain (https://rustup.rs). The GUI uses the `glow` (OpenGL)
backend for broad compatibility, including WSLg. See the
[Development Guide](./development-guide.md) for details.

## Using this index for AI-assisted work

- **Numeric / logic changes:** start at [Architecture — Core](./architecture-core.md)
  and [API Contracts — Core](./api-contracts-core.md).
- **UI changes:** start at [Architecture — GUI](./architecture-gui.md) and
  [Component Inventory — GUI](./component-inventory-gui.md).
- **Cross-cutting changes:** also read
  [Integration Architecture](./integration-architecture.md).
