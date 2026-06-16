# Integration Architecture (core ↔ gui)

_Generated: 2026-06-16 · Scan level: exhaustive_

PowerCalc is a two-part Cargo workspace. This document describes how the parts
connect. The integration is **in-process Rust**: there is no network, IPC, or
serialization boundary — `powercalc-gui` links `powercalc-core` directly.

## Integration points

| From | To | Type | Details |
|------|----|------|---------|
| `powercalc-gui` | `powercalc-core` | Cargo path dependency | `powercalc-core = { path = "../core" }` in `crates/gui/Cargo.toml`. |
| `app.rs` | `core` public API | Direct function/method calls | `use powercalc_core::{eval, fixed, Signedness, Value, Width};` |

## Dependency direction

```
powercalc-gui  ──depends on──▶  powercalc-core
   (desktop UI)                   (pure numeric library, no deps)
```

The dependency is strictly one-way. `powercalc-core` knows nothing about the GUI
and has no UI dependencies, which is what lets the entire numeric test suite run
without a display.

## Data flow

A single `Value` (held in `App`) is the shared currency. Every interaction follows
the same loop: **mutate the canonical `Value` via core → `refresh()` rewrites the
text buffers → the next frame redraws all views from `Value`.**

```
                 ┌──────────────────────── App state (gui) ─────────────────────────┐
   user input    │  value: Value   width: Width   sign: Signedness   frac_bits      │
   ───────────▶  │  text buffers: hex/dec/bin/oct/fixed_input/expr                   │
                 └───────────────────────────────┬──────────────────────────────────┘
                                                  │ calls into core
                 ┌────────────────────────────────▼─────────────────────────────────┐
   Expression line ──▶ core::eval(expr, width, sign, ans=value) ──▶ Result<Value>    │
   Base field edit ──▶ gui parse_base(...) ──▶ Value::new(raw, width)                 │
   Fixed "Real"   ──▶ core::fixed::from_real(real, width, frac_bits) ──▶ Value       │
   Bit click      ──▶ widgets::bit_grid(...) ──▶ value.with_raw(raw ^ (1<<b))         │
   Width preset   ──▶ Value::with_width(width)                                        │
   Sign toggle    ──▶ (re-render only; affects to_dec / >> / div)                     │
                 └────────────────────────────────┬─────────────────────────────────┘
                                                  │ new Value
                 ┌────────────────────────────────▼─────────────────────────────────┐
   refresh(skip) rewrites buffers via Value::to_hex/to_bin/to_oct/to_dec(sign) and   │
   fixed::to_real; views redraw next frame (bit grid reads value.raw()).             │
                 └──────────────────────────────────────────────────────────────────┘
```

## Core API used by the GUI

| GUI trigger | Core call |
|-------------|-----------|
| Evaluate expression | `eval(&expr, width, sign, value)` |
| Render decimal | `value.to_dec(sign)` |
| Render hex/bin/oct | `value.to_hex()` / `to_bin()` / `to_oct()` |
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
- `cargo run -p powercalc-gui` / `cargo build --release` build the linked binary
  `powercalc`.

See the [Development Guide](./development-guide.md) for commands and the
[GUI architecture](./architecture-gui.md) / [Core architecture](./architecture-core.md)
for each side in detail.
