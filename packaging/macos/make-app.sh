#!/usr/bin/env bash
# Builds Marquee.app (with its icon) into ./dist. Usage: packaging/macos/make-app.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --release
APP=dist/Marquee.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/marquee "$APP/Contents/MacOS/Marquee"
cp target/release/marquee-share "$APP/Contents/MacOS/marquee-share"

# .icns from the rendered PNGs
SET=$(mktemp -d)/Marquee.iconset
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  cp "assets/appicon/marquee-$s.png" "$SET/icon_${s}x${s}.png"
done
cp assets/appicon/marquee-32.png   "$SET/icon_16x16@2x.png"
cp assets/appicon/marquee-64.png   "$SET/icon_32x32@2x.png"
cp assets/appicon/marquee-256.png  "$SET/icon_128x128@2x.png"
cp assets/appicon/marquee-512.png  "$SET/icon_256x256@2x.png"
cp assets/appicon/marquee-1024.png "$SET/icon_512x512@2x.png"
iconutil -c icns "$SET" -o "$APP/Contents/Resources/Marquee.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Marquee</string>
  <key>CFBundleDisplayName</key><string>Marquee</string>
  <key>CFBundleIdentifier</key><string>org.marquee.app</string>
  <key>CFBundleExecutable</key><string>Marquee</string>
  <key>CFBundleIconFile</key><string>Marquee</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.entertainment</string>
</dict></plist>
PLIST
echo "Built $APP"
