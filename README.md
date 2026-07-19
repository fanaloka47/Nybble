# Nybble

Tired of the Windows calculator ? Here is Nybble, a calculator targeted at people often needing to switch or mix bases (like FPGA engineers).

<p align="center">
  <img src="assets/screenshot.png" alt="Nybble showing a value in HEX, DEC, and BIN with a clickable bit grid" width="380">
</p>

## What it does

- **All bases at once.** Edit HEX, DEC, BIN, or OCT and the rest update as you type. No
  convert button, no mode switch.
- **Width and signedness.** 8/16/32/64-bit presets or any custom width (1–128), unsigned
  or two's-complement. Truncation and arithmetic vs. logical `>>` match what the hardware
  actually does, so the decimal you see is the decimal you'd get.
- **Clickable bits.** The grid runs MSB→LSB, grouped by nibble — click a bit and
  everything updates.
- **Real expressions.** Type `0xFF & (1 << 3)`, mix bases (`0x`, `0b`, `0o`, decimal) with
  `_` separators, and reuse the last result as `ans`. There's `**` for powers and the
  functions you'd reach for — `sqrt`, `log2`, `clog2`, `gcd`.
- **Float mode** when you've left the integers behind: the usual scientific set (`sin`,
  `cos`, `log`, …) plus `pi`, `e`, `tau`.
- **Fixed-point (Qm.n).** A view, not a math mode — read or enter the current value as a
  fixed-point real (the raw bits stay exact). Arithmetic happens on the integer bits or in
  float mode.

## Download & install

Grab the latest build from the [Releases page](https://github.com/fanaloka47/nybble/releases).
Every flavor is the same app — pick whichever suits you.

| Platform | Download | Notes |
|---|---|---|
| Windows | `Nybble-<ver>-x64.msi` | Installs to Program Files, with Start Menu and Add/Remove entries |
| Windows (portable) | `nybble-x86_64-pc-windows-msvc.zip` | Unzip and run, no install |
| Linux | `nybble_<ver>_amd64.deb` | Debian/Ubuntu/Mint/Pop!_OS; adds a launcher entry |
| Linux (universal) | `Nybble-<ver>-x86_64.AppImage` | `chmod +x` and run on any distro |
| Linux (portable) | `nybble-x86_64-unknown-linux-gnu.tar.gz` | Untar and run |

> The Windows builds are unsigned, so the first launch (or the installer) shows a
> SmartScreen "unknown publisher" prompt — choose **More info → Run anyway**.

**Settings and history are shared across flavors.** They live in
`%APPDATA%\Nybble` and `~/.local/share/nybble`, so installing the MSI over a
portable copy keeps your history, and uninstalling never deletes it.

### Updating

The app checks GitHub for newer releases on launch. What it offers depends on
how you installed it:

- **Portable and AppImage** — one-click **Update & restart**; it replaces itself.
- **MSI** — downloads the new installer and runs it; Windows asks to elevate,
  then the app reopens.
- **`.deb`** — tells you a version is available and links to the Releases page.
  It never writes to `/usr/bin` behind your package manager's back.

Linux builds use X11, so they run through XWayland on Wayland desktops.

## Build from source

Requires a [Rust toolchain](https://rustup.rs):

```sh
cargo run -p nybble-gui      # launch the app
cargo build --release        # produce target/release/nybble
cargo test                   # run the core test suite
```

## Project layout

```
crates/
  core/   nybble-core — pure, UI-free numeric logic (fully unit-tested)
  gui/    nybble-gui  — eframe/egui desktop app (binary: `nybble`)
```

All number logic lives in `core` and is tested without the GUI; the GUI is a thin
presentation layer over it.

## Notes & limits

- Values are capped at **128 bits** for now. Wider buses (256/512-bit) would need an
  arbitrary-precision backend — a possible future extension.
- Fixed-point conversion goes through `f64`, so very wide values or many fractional bits
  can lose precision in the *displayed* real (the raw bits remain exact).
</content>
