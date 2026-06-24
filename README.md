# Nybble

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
  core/   nybble-core — pure, UI-free numeric logic (fully unit-tested)
  gui/    nybble-gui  — eframe/egui desktop app (binary: `nybble`)
```

All number logic lives in `core` and is tested without the GUI. The GUI is a thin layer
over it.

## Build & run

Requires a Rust toolchain (https://rustup.rs). Then:

```sh
cargo run -p nybble-gui      # launch the app
cargo test                   # run the core test suite
cargo build --release        # produce target/release/nybble
```

The GUI uses the `glow` (OpenGL) backend for broad compatibility, including WSLg.

## Releasing (Windows)

1. Bump `version` in `crates/gui/Cargo.toml` (e.g. `0.1.0` → `0.1.1`).
2. Commit. Then:
   ```sh
   git tag v0.1.1
   git push origin main --tags
   ```
3. CI builds `nybble.exe`, zips it as `nybble-x86_64-pc-windows-msvc.zip`, and
   publishes a GitHub Release with auto-generated notes.
4. Colleagues' apps detect the new release on next launch and offer a one-click update.

The tag version must match `Cargo.toml`; the CI guard rejects mismatches before building.

## Auto-updating

On launch, the app silently checks GitHub Releases. If a newer version exists, a
**"Update & restart"** button appears in the top-right corner. Clicking it downloads the
new binary, swaps it in, and restarts the app. The check can be disabled from the
settings (persisted across sessions).

A **"Check for updates"** button is always available in the header when no check is
currently running.

## Notes / limits

- Values are capped at **128 bits** for now. Wider buses (256/512-bit) would need an
  arbitrary-precision backend — a possible future extension.
- Fixed-point conversion goes through `f64`, so very wide values or many fractional bits
  can lose precision in the *displayed* real (the raw bits remain exact).
- The Windows binary is unsigned; the first run shows a SmartScreen "unknown publisher"
  prompt (More info → Run anyway). Acceptable for internal colleagues.
