#!/usr/bin/env bash
# Build Nybble.AppImage from an already-compiled release binary.
#
# Run from the repo root after `cargo build --release -p nybble-gui`.
# Produces ./Nybble-<version>-x86_64.AppImage.
#
# Deliberately does NOT bundle libGL/libX11: those must come from the host, or
# the AppImage works only on machines with the same graphics stack as the
# builder. linuxdeploy's default exclude list handles this for us.
set -euo pipefail

cd "$(dirname "$0")/.."

BIN=target/release/nybble
[ -x "$BIN" ] || { echo "missing $BIN — run: cargo build --release -p nybble-gui" >&2; exit 1; }

VERSION=$(grep -m1 '^version' crates/gui/Cargo.toml | cut -d'"' -f2)
ARCH=${ARCH:-x86_64}
APPDIR=target/appimage/Nybble.AppDir

rm -rf "$APPDIR"
install -Dm755 "$BIN"                       "$APPDIR/usr/bin/nybble"
install -Dm644 packaging/nybble.desktop     "$APPDIR/usr/share/applications/nybble.desktop"
for size in 16 32 48 64 128 256; do
    install -Dm644 "packaging/icons/hicolor/${size}x${size}/apps/nybble.png" \
        "$APPDIR/usr/share/icons/hicolor/${size}x${size}/apps/nybble.png"
done

# linuxdeploy also wants the desktop file and icon at the AppDir root.
install -Dm644 packaging/icons/hicolor/256x256/apps/nybble.png "$APPDIR/nybble.png"

TOOL=target/appimage/linuxdeploy-$ARCH.AppImage
if [ ! -x "$TOOL" ]; then
    mkdir -p target/appimage
    curl -fsSL -o "$TOOL" \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$ARCH.AppImage"
    chmod +x "$TOOL"
fi

# linuxdeploy is itself an AppImage; on hosts without FUSE (containers, some CI
# runners) it has to be run extracted.
if ! "$TOOL" --list-plugins >/dev/null 2>&1; then
    export APPIMAGE_EXTRACT_AND_RUN=1
fi

# linuxdeploy reads both of these from the environment, so they have to be
# exported rather than passed as an assignment prefix (in a prefix list, later
# assignments aren't visible to earlier expansions).
export VERSION
export OUTPUT="Nybble-$VERSION-$ARCH.AppImage"

"$TOOL" --appdir "$APPDIR" \
    --desktop-file "$APPDIR/usr/share/applications/nybble.desktop" \
    --icon-file "$APPDIR/nybble.png" \
    --output appimage

echo "built Nybble-$VERSION-$ARCH.AppImage"
