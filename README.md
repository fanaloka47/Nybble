# PowerCalc

A fast desktop calculator for working across number bases — built for FPGA/hardware
work where you constantly move between hex, decimal, and binary, care about bit width
and signedness, and want to poke at individual bits.

## Features

- **Live multi-base fields** — edit HEX, DEC, BIN, or OCT and the others update instantly.
- **Width & signedness** — 8/16/32/64-bit presets or any custom width (1–128); unsigned or
  two's-complement signed (affects decimal display and arithmetic vs logical `>>`).
- **Interactive bit grid** — click any bit (MSB→LSB) and watch every base update.
- **Expressions** — type things like `0xFF & (1 << 3)`, with `ans` for the current value;
  mixed-base literals (`0x`, `0b`, `0o`, decimal) and `_` separators are accepted.
- **Bitwise/arithmetic ops** — `& | ^ ~ << >> + - * / %`, plus operator buttons.
- **Fixed-point (Qm.n)** — view/enter the value as a fixed-point real for DSP work.

## Project layout

```
crates/
  core/   powercalc-core — pure, UI-free numeric logic (fully unit-tested)
  gui/    powercalc-gui  — eframe/egui desktop app (binary: `powercalc`)
```

All number logic lives in `core` and is tested without the GUI. The GUI is a thin layer
over it.

## Build & run

Requires a Rust toolchain (https://rustup.rs). Then:

```sh
cargo run -p powercalc-gui      # launch the app
cargo test                      # run the core test suite
cargo build --release           # produce target/release/powercalc
```

The GUI uses the `glow` (OpenGL) backend for broad compatibility, including WSLg.

## Notes / limits

- Values are capped at **128 bits** for now. Wider buses (256/512-bit) would need an
  arbitrary-precision backend — a possible future extension.
- Fixed-point conversion goes through `f64`, so very wide values or many fractional bits
  can lose precision in the *displayed* real (the raw bits remain exact).
