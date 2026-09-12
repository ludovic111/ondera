#!/usr/bin/env bash
# Wrap target/release/ondera into dist/Ondera.app and zip it as dist/Ondera-macos-<arch>.zip,
# the asset name the in-app updater downloads. The bundle's version follows Cargo.toml.
set -euo pipefail
cd "$(dirname "$0")/.."
test -x target/release/ondera || { echo 'Run cargo build --release first.' >&2; exit 1; }
version=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
case "$(uname -m)" in
  arm64|aarch64) arch=arm64 ;;
  x86_64) arch=x86_64 ;;
  *) echo "Unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
bundle='dist/Ondera.app'
rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp target/release/ondera "$bundle/Contents/MacOS/ondera"
sed -e "s|<string>0\.0\.0</string>|<string>$version</string>|" desktop/Info.plist > "$bundle/Contents/Info.plist"
cp desktop/assets/Ondera.icns "$bundle/Contents/Resources/Ondera.icns"
# Ad-hoc signing for local testing and self-updates. Public Developer ID signing and
# notarization are a separate step that needs the owner's certificate.
codesign --force --deep --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
rm -f "dist/Ondera-macos-$arch.zip"
ditto -c -k --sequesterRsrc --keepParent "$bundle" "dist/Ondera-macos-$arch.zip"
echo "Built $bundle ($version, $arch) and dist/Ondera-macos-$arch.zip"
