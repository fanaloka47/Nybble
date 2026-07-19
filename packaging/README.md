# Packaging

Nybble ships in four flavors. CI (`.github/workflows/release.yml`) builds all of
them on tag push; the commands below are for reproducing an artifact locally.

| Artifact | Built by | Installs to |
|---|---|---|
| `nybble-x86_64-pc-windows-msvc.zip` | `7z` in CI | wherever the user unzips it |
| `Nybble-<ver>-x64.msi` | `cargo wix` + `crates/gui/wix/main.wxs` | `C:\Program Files\Nybble\` |
| `nybble-x86_64-unknown-linux-gnu.tar.gz` | `tar` in CI | wherever the user untars it |
| `nybble_<ver>_amd64.deb` | `cargo deb` + `[package.metadata.deb]` | `/usr/bin/nybble` |
| `Nybble-<ver>-x86_64.AppImage` | `packaging/build-appimage.sh` | run in place |

## Asset naming is load-bearing

`self_update` picks a release asset by substring-matching the Rust target
triple. Two rules follow, and breaking either one breaks the in-app updater:

- The **portable archives must keep their triple** (`nybble-x86_64-pc-windows-msvc.zip`,
  `nybble-x86_64-unknown-linux-gnu.tar.gz`). Renaming them strands every
  existing portable user.
- The **MSI must not contain a triple**. If it did, `self_update` could match
  the installer and try to execute it as the application binary. It is selected
  by extension instead — see `pick_asset` in `crates/gui/src/update.rs`.

## Linux

```sh
cargo build --release -p nybble-gui

cargo install cargo-deb
cargo deb -p nybble-gui --no-build      # → target/debian/nybble_<ver>_amd64.deb

bash packaging/build-appimage.sh        # → Nybble-<ver>-x86_64.AppImage
```

Build on the **oldest** distribution you intend to support (CI uses
`ubuntu-22.04`). glibc is only forward compatible, so the build machine sets the
floor for which systems can run the result. This is not theoretical: a `.deb`
built on Ubuntu 24.04 declares `libc6 (>= 2.39)` and flatly refuses to install
on 22.04, which ships 2.35.

### Testing a package without installing it on your machine

```sh
podman run --rm -v "$PWD/target/debian:/pkg:ro" ubuntu:22.04 bash -c \
  'apt-get update -qq && apt-get install -y /pkg/nybble_*.deb && /usr/bin/nybble'
```

A **display** error (`neither WAYLAND_DISPLAY nor ... nor DISPLAY is set`) is the
success case in a headless container: it means every `dlopen`ed library resolved
and the app got as far as opening a window. A *library* error means the
dependency list above is missing something.

Note that minimal container images set `path-exclude=/usr/share/man/*` in
`/etc/dpkg/dpkg.cfg.d/`, so the man page legitimately won't appear there. Use
`dpkg --path-include="/usr/share/man/*" -i` to check it.

`lintian target/debian/nybble_*.deb` should report only
`initial-upload-closes-no-bugs`, which applies to uploads into the Debian
archive (it wants an ITP bug number) and is irrelevant here.

### Runtime dependencies are not auto-detected

`depends = "$auto"` alone yields only `libc6`, because eframe reaches X11 and
OpenGL through `dlopen` rather than `DT_NEEDED` — and `dpkg-shlibdeps` cannot
see `dlopen`. The explicit list in `crates/gui/Cargo.toml` covers the rest.
If eframe/winit ever changes which libraries it loads, refresh it with:

```sh
strings -a target/release/nybble | grep -oE 'lib[A-Za-z0-9_-]+\.so(\.[0-9]+)?' | sort -u
```

Note also that eframe is built with the `x11` feature only (`wayland` crashes
under WSL), so both Linux artifacts run through XWayland on Wayland desktops.

## Windows

```sh
cargo install cargo-wix
cargo wix -p nybble-gui --nocapture --output Nybble-<ver>-x64.msi
```

Requires the **WiX Toolset v3** (`candle.exe`/`light.exe`) on `PATH`, via `$WIX`,
or passed with `--bin-path`. WiX 4/5 use a different schema and will not build
`main.wxs`.

### Paths inside main.wxs

All file references are anchored to `$(sys.SOURCEFILEDIR)` — the `.wxs` file's
own directory — which candle expands to an absolute path. Use it for anything
new. Plain relative paths are a trap here, because the two WiX tools disagree
about what they are relative to:

- `cargo-wix` invokes `light -b <package dir>`, so `File/@Source` and
  `Icon/@SourceFile` resolve against `crates/gui`.
- A `WixVariable` (such as `WixUILicenseRtf`) ignores `-b` entirely and resolves
  against the **process working directory** — wherever `cargo wix` was run from.

So a relative license path that works locally fails in CI, or vice versa.

`crates/gui/wix/main.wxs` is committed rather than generated. `cargo wix init`
would regenerate it with a **fresh `UpgradeCode`**, and that GUID is what makes a
new version replace the old one instead of installing beside it. Never change:

- `UpgradeCode` on `<Product>`
- the explicit component GUIDs

`License.rtf` is generated from the repo-root `LICENSE`; regenerate it if the
license ever changes.

### Expected build warning

`light` emits `LGHT1076 : ICE61 ... The Maximum version is not less than the
current product` on every build. That is the documented consequence of
`AllowSameVersionUpgrades="yes"`, which deliberately includes the current
version in the upgrade range so reinstalling the same MSI is idempotent rather
than an error. It is a warning, not a failure — don't "fix" it by dropping the
attribute.

### Shortcut components must use HKCU

A shortcut cannot be its own keypath, so both shortcut components anchor on a
registry value — and that value has to live under `HKCU`, even though this
package installs per-machine. Windows Installer classifies shortcut folders as
user profile data regardless of `ALLUSERS`, so:

- `HKCU` — accepted
- `HKLM` — ICE57: "per-user and per-machine data with a per-machine KeyPath"
- `HKMU` — ICE57: keypath "can be either per-user or per-machine"

ICE38 and ICE43 independently require `HKCU` for non-advertised shortcuts. The
trade-off is that shortcut state is tracked per user, so a second user on the
same machine may need an installer repair to get their own copies.

### Signing

The MSI is unsigned, so Windows SmartScreen shows a "Windows protected your PC"
interstitial on first run. Removing that needs an OV or EV code-signing
certificate. Until then it is worth saying so in the README so the warning isn't
read as malware.

## Release channels and testing a candidate

The updater has two channels, selected by `PC_UPDATE_CHANNEL`:

| Channel | Env | Endpoint | Sees pre-releases |
|---|---|---|---|
| Stable (default) | unset | `/releases/latest` | no — GitHub filters server-side |
| Beta | `beta` | `/releases` (list) | yes, newest first |

Stable defers to GitHub's own definition of "latest" (most recent non-draft,
non-pre-release) rather than guessing from the tag name, so the **GitHub
pre-release flag is what keeps an RC away from users** — not the `-rc.N` suffix.
A candidate published as a normal release is offered to everyone.

The release workflow now sets that flag itself: the `guard` job marks any
version containing a hyphen as a pre-release, matching semver. It was a manual
checkbox until `v1.5.1-rc.1` shipped without it and briefly became the "latest"
release for every stable user, so it is derived from the version now rather than
remembered.

`apply_update` pins the download to the exact tag the check found. Without that
pin the download always resolves `/releases/latest`, so a beta user offered an RC
would silently receive the stable build instead.

### What existing 1.4.0 users see

1.4.0 shipped before any of this and checks the unfiltered `/releases` list, so
it cannot be protected retroactively — its logic is already on user machines.
With `v1.5.0` published and `v1.6.0-rc.1` marked pre-release, a 1.4.0 user:

1. is offered **"Update & restart (v1.6.0-rc.1)"** — the wrong label
2. clicks it; the download path asks for `/releases/latest` and gets **v1.5.0**
3. installs 1.5.0 and restarts — the correct build, despite the label
4. from then on runs filtered logic and stops seeing pre-releases entirely

So the mislabel self-heals after one upgrade. Publishing the RC *before* the
stable release avoids even that, since the list's newest entry would then be
1.5.0.

One caveat: 1.4.0's failure handling leaves the button stuck on "Updating…" if a
download fails (fixed in 1.5.0). **Verify every release asset is present before
announcing 1.5.0** — those users get one clean attempt at it.

### Suggested sequence

1. Publish `v1.5.0` alone. Confirm a real 1.4.0 install upgrades to it.
2. Publish `v1.6.0-rc.1` as a pre-release. Confirm a stock 1.5.0 client
   ignores it and `PC_UPDATE_CHANNEL=beta` sees and installs it.

Testing them separately means a failure tells you which half broke.

## User data is never removed

Both installers deliberately leave `%APPDATA%\Nybble` and
`~/.local/share/nybble` alone on uninstall, and both flavors share those paths
with the portable build (the app id is pinned in `crates/gui/src/main.rs`). That
is what lets someone install the MSI over a portable copy and keep their history,
and what makes an uninstall/reinstall non-destructive.
