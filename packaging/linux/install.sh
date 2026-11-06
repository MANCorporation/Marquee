#!/usr/bin/env bash
# Installs Marquee for the current user: binaries in ~/.local/bin, icon and launcher entry (GNOME/KDE menus).
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --release
PREFIX="${PREFIX:-$HOME/.local}"
install -Dm755 target/release/marquee "$PREFIX/bin/marquee"
install -Dm755 target/release/marquee-share "$PREFIX/bin/marquee-share"
install -Dm644 assets/appicon/marquee.svg "$PREFIX/share/icons/hicolor/scalable/apps/marquee.svg"
for s in 16 32 64 128 256 512; do
  install -Dm644 "assets/appicon/marquee-$s.png" "$PREFIX/share/icons/hicolor/${s}x${s}/apps/marquee.png"
done
install -Dm644 packaging/linux/marquee.desktop "$PREFIX/share/applications/marquee.desktop"
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$PREFIX/share/icons/hicolor" || true
command -v update-desktop-database >/dev/null && update-desktop-database -q "$PREFIX/share/applications" || true
echo "Installed to $PREFIX"
