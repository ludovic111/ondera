#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
test -x target/release/ondera || { echo 'Run cargo build --release first.' >&2; exit 1; }
bundle='dist/Ondera.app'
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp target/release/ondera "$bundle/Contents/MacOS/ondera"
cp desktop/Info.plist "$bundle/Contents/Info.plist"
# Ad-hoc signing for local testing. Public Developer ID signing/notarization is separate.
codesign --force --deep --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
ditto -c -k --sequesterRsrc --keepParent "$bundle" dist/Ondera-macos.zip
echo "Built $bundle and dist/Ondera-macos.zip"
