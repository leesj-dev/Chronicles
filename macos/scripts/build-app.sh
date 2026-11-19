#!/bin/bash
set -euo pipefail
MAC_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$MAC_ROOT/.." && pwd)"
CONFIGURATION="${1:-release}"
if [[ "$CONFIGURATION" != debug && "$CONFIGURATION" != release ]]; then
    echo "Usage: $0 [debug|release]" >&2
    exit 2
fi
swift build --package-path "$MAC_ROOT" -c "$CONFIGURATION"
BIN_DIR="$(swift build --package-path "$MAC_ROOT" -c "$CONFIGURATION" --show-bin-path)"
APP_PATH="$MAC_ROOT/build/Chronicles.app"
mkdir -p "$APP_PATH/Contents/MacOS" "$APP_PATH/Contents/Resources"
cp "$BIN_DIR/Chronicles" "$APP_PATH/Contents/MacOS/Chronicles"
cp "$REPO_ROOT/examples/demo.json" "$APP_PATH/Contents/Resources/demo.json"
cp "$MAC_ROOT/Resources/Geist.ttf" "$APP_PATH/Contents/Resources/Geist.ttf"
cp "$MAC_ROOT/Resources/Geist-OFL.txt" "$APP_PATH/Contents/Resources/Geist-OFL.txt"
cat > "$APP_PATH/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Chronicles</string>
<key>CFBundleDisplayName</key><string>Chronicles</string>
<key>CFBundleIdentifier</key><string>dev.leesj.chronicles</string>
<key>CFBundleExecutable</key><string>Chronicles</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
codesign --force --sign - "$APP_PATH"
echo "$APP_PATH"
