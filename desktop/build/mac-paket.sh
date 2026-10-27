#!/usr/bin/env bash
# Baut aus dem zusammengestellten Ordner das Mac-Programm (OpenAgenticOffice.app) und ein Disk-Image.
#   desktop/build/mac-paket.sh <quellordner> <version> <arch>      (arch: arm64 oder x64)
# Der Quellordner enthält die Desktop-Hülle, den Server, dist/, claude/ und tools/.
# Die App wird ad hoc signiert (ohne Apple-Zertifikat), das braucht Apple Silicon zum Starten.
set -euo pipefail
quelle="$1"; version="$2"; arch="$3"
app="desktop/release/OpenAgenticOffice.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" desktop/release
cp -R "$quelle"/. "$app/Contents/MacOS/"
cp desktop/build/icon.png "$app/Contents/Resources/icon.png"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>OpenAgenticOffice</string>
  <key>CFBundleDisplayName</key><string>OpenAgenticOffice</string>
  <key>CFBundleIdentifier</key><string>io.github.luna-os.openagenticoffice</string>
  <key>CFBundleExecutable</key><string>open-agentic-office-desktop</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
chmod +x "$app/Contents/MacOS/"* 2>/dev/null || true
codesign --force --deep --sign - "$app"
dmg="desktop/release/OpenAgenticOffice-${version}-mac-${arch}.dmg"
rm -f "$dmg"
hdiutil create -volname "OpenAgenticOffice" -srcfolder "$app" -ov -format UDZO "$dmg"
echo "$dmg"
