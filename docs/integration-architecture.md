# Integration Architecture (core ↔ gui)

_Generated: 2026-06-16 · Updated: 2026-06-23 (full rescan) · Scan level: exhaustive_

nybble is a two-part Cargo workspace. This document describes how the parts
connect. The integration is **in-process Rust**: there is no network, IPC, or
serialization boundary — `nybble-gui` links `nybble-core` directly.

## Integration points

| From | To | Type | Details |
|------|----|------|---------|
| `nybble-gui` | `nybble-core` | Cargo path dependency | `nybble-core = { path = "../core" }` in `crates/gui/Cargo.toml`. |
| `app.rs` | `core` public API | Direct function/method calls | `use nybble_core::{eval, eval_float, f64_to_value, fixed, Signedness, Value, Width};` |

## Dependency direction

```
nybble-gui  ──depends on──▶  nybble-core
   (desktop UI)                   (pure numeric library, no deps)
```

The dependency is strictly one-way. `nybble-core` knows nothing about the GUI
and has no UI dependencies, which is what lets the entire numeric test suite run
without a display.

## Data flow

A single `Value` (held in `App`) is the shared currency in **integer mode**. Every
interaction follows the same loop: **mutate the canonical `Value` via core →
`refresh()` rewrites the text buffers → the next frame redraws all views from
`Value`.** In **float mode** the parallel `float_value: f64` is the currency
instead, evaluated via `core::eval_float` and rendered through `f64_to_value` for
the bit bases.

```
                 ┌──────────────────────── App state (gui) ─────────────────────────┐
   user input    │  value: Value   width   sign   frac_bits   float_value: f64       │
   ───────────▶  │  number_mode: Integer | Float                                     │
                 │  text buffers: hex/dec/bin/oct/fixed_input/expr                   │
                 └───────────────────────────────┬──────────────────────────────────┘
                                                  │ calls into core
                 ┌────────────────────────────────▼─────────────────────────────────┐
   Expression (int)  ──▶ core::eval(expr, width, sign, ans=value) ──▶ Result<Value>  │
   Expression (float)──▶ core::eval_float(expr, ans=float_value) ──▶ Result<f64>     │
   Base field edit   ──▶ gui parse_base(...) ──▶ Value::new(raw, width)              │
   Fixed "real"      ──▶ core::fixed::from_real(real, width, frac_bits) ──▶ Value    │
   Bit click         ──▶ widgets::bit_grid(...) ──▶ value.with_raw(raw ^ (1<<b))     │
   Width preset/drag ──▶ Value::with_width(width)                                    │
   Sign toggle       ──▶ (re-render only; affects to_dec / >> / div)                 │
                 └────────────────────────────────┬─────────────────────────────────┘
                                                  │ new Value / f64
                 ┌────────────────────────────────▼─────────────────────────────────┐
   refresh(skip): integer mode → Value::to_hex/to_bin/to_oct/to_dec(sign) +          │
   fixed::to_real; float mode → format!(f64) for DEC and f64_to_value(x).to_hex/…    │
   for the bit bases. Views redraw next frame (bit grid reads value.raw()).          │
                 └──────────────────────────────────────────────────────────────────┘
```

## Core API used by the GUI

| GUI trigger | Core call |
|-------------|-----------|
| Evaluate expression (integer mode) | `eval(&expr, width, sign, value)` |
| Evaluate expression (float mode) | `eval_float(&expr, float_value)` |
| Render integer decimal | `value.to_dec(sign)` |
| Render integer hex/bin/oct | `value.to_hex()` / `to_bin()` / `to_oct()` |
| Render float bit bases | `f64_to_value(x).to_hex()` / `to_bin()` / `to_oct()` |
| Fixed-point display | `fixed::to_real(value, frac_bits, sign)` |
| Fixed-point entry | `fixed::from_real(real, width, frac_bits)` |
| Change width | `value.with_width(Width::clamped(bits))` |
| Bit toggle | `value.with_raw(value.raw() ^ (1 << b))` |

Note: the GUI does its own base-field parsing in `app.rs::parse_base` (to support a
signed leading `-` and tolerant whitespace), rather than calling
`core::parse_literal` directly. `parse_literal` is used inside `core::expr` for
expression literals.

## Build & test boundary

- `cargo test` runs the **core** suite (the GUI has no tests).
- `cargo run -p nybble-gui` / `cargo build --release` build the linked binary
  `nybble`.

See the [Development Guide](./development-guide.md) for commands and the
[GUI architecture](./architecture-gui.md) / [Core architecture](./architecture-core.md)
for each side in detail.
