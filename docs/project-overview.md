# nybble — Project Overview

_Generated: 2026-06-16 · Updated: 2026-06-23 (full rescan) · Scan level: exhaustive_

## Purpose

nybble is a **fast native desktop calculator for working across number bases**,
built for FPGA / hardware engineers who constantly move between hexadecimal,
decimal, binary and octal, care about **bit width** and **signedness**, and want
to poke at **individual bits**. The product goal (see [`cdc.txt`](../cdc.txt) and
[`nybble-plan.md`](./nybble-plan.md)) is to make *playing with number
representations* fluid: see one value in every base at once, flip bits, choose
widths/signedness, and run bitwise/arithmetic expressions — on Windows and Linux.

## Executive summary

The codebase is a small (~3,200 LOC) **Rust Cargo workspace** with a deliberate
two-layer split:

- **`nybble-core`** — a pure, UI-free numeric library. It holds the canonical
  value model (a width-masked `u128`), all width-correct operations, literal
  parsing, a hand-rolled integer expression evaluator (tokenizer + Pratt parser),
  a parallel full-precision **`f64` evaluator** (float mode), and fixed-point
  (Qm.n) conversion. It has **no GUI dependencies** and carries the entire test
  suite (43 unit tests).
- **`nybble-gui`** — a thin [`eframe`](https://crates.io/crates/eframe) /
  [`egui`](https://github.com/emilk/egui) 0.34 immediate-mode desktop app. It
  owns application state and rendering only; every numeric decision is delegated
  to `core`. The binary is named `nybble`.

Design rule enforced throughout: **all numeric logic lives in `core` and is unit-
tested without the GUI.** The GUI is a presentation layer over the library.

## Tech stack summary

| Category        | Technology                         | Version  | Notes |
|-----------------|------------------------------------|----------|-------|
| Language        | Rust (edition 2021)                | —        | Workspace, resolver 2 |
| Core library    | `nybble-core` (no deps)         | 0.1.0    | Dependency-free numeric logic |
| GUI framework   | `eframe` / `egui`                  | 0.34     | Immediate-mode GUI |
| Render backend  | `glow` (OpenGL)                    | —        | Chosen for WSLg/broad compatibility |
| Windowing       | `winit` via eframe (x11 only)      | —        | `wayland` & `accesskit` disabled (WSL stability / D-Bus) |
| Persistence     | eframe `persistence` feature       | —        | Stores theme, history-base, view-mode, number-mode, custom size |
| Build / test    | Cargo                              | —        | `cargo test`, `cargo build --release` |

## Architecture type

- **Repository type:** monorepo (Cargo workspace, 2 members)
- **Pattern:** layered — pure domain core + thin immediate-mode UI shell
- **Canonical state:** a single `Value` (raw `u128` masked to a `Width` of 1..=128)
  drives every view; `Signedness` is applied only at decimal-render and
  shift/division time, never stored on the value.

## Parts

| Part   | Crate            | Type     | Root                | Role |
|--------|------------------|----------|---------------------|------|
| core   | `nybble-core` | library  | `crates/core`       | Numeric logic, fully tested |
| gui    | `nybble-gui`  | desktop  | `crates/gui`        | eframe/egui app, binary `nybble` |

## Documentation map

- [Source Tree Analysis](./source-tree-analysis.md)
- [Architecture — Core](./architecture-core.md)
- [Architecture — GUI](./architecture-gui.md)
- [API Contracts — Core (public library API)](./api-contracts-core.md)
- [Component Inventory — GUI](./component-inventory-gui.md)
- [Integration Architecture (core ↔ gui)](./integration-architecture.md)
- [Development Guide](./development-guide.md)
- [Master Index](./index.md)

## Known limits

- Values are capped at **128 bits**. Wider buses (256/512-bit) would need an
  arbitrary-precision backend — noted as a future extension.
- Fixed-point conversion goes through `f64`, so very wide values or many
  fractional bits can lose precision in the *displayed* real; the raw bits stay
  exact.
