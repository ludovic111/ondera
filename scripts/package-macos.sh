#!/usr/bin/env bash
# Wrap target/release/ondera into dist/Ondera.app and zip it as dist/Ondera-macos-<arch>.zip,
# the asset name the in-app updater downloads. The bundle's version follows Cargo.toml.
set -euo pipefail
cd "$(dirname "$0")/.."
test -x target/release/ondera || { echo 'Run cargo build --release --workspace first.' >&2; exit 1; }
version=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
case "$(uname -m)" in
  arm64|aarch64) arch=arm64 ;;
  x86_64) arch=x86_64 ;;
  *) echo "Unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
# Validate every companion before replacing an existing local package.
for binary in ondera ondera-cli ondera-mcp; do
  test -x "target/release/$binary" || { echo "Missing $binary; build the workspace first." >&2; exit 1; }
  actual=$("target/release/$binary" --version)
  test "$actual" = "$binary $version" || { echo "$binary has stale version: $actual (expected $version)" >&2; exit 1; }
  lipo "target/release/$binary" -verify_arch "$arch"
done
bundle='dist/Ondera.app'
rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
for binary in ondera ondera-cli ondera-mcp; do
  cp "target/release/$binary" "$bundle/Contents/MacOS/$binary"
done
sed -e "s|<string>0\.0\.0</string>|<string>$version</string>|" desktop/Info.plist > "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$bundle/Contents/Info.plist"
plutil -lint "$bundle/Contents/Info.plist"
cp desktop/assets/Ondera.icns "$bundle/Contents/Resources/Ondera.icns"
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  # Developer ID: hardened runtime, secure timestamp, companions first and the bundle last.
  for binary in ondera-cli ondera-mcp ondera; do
    target="$bundle/Contents/MacOS/$binary"
    test "$binary" = ondera && target="$bundle"
    codesign --force --options runtime --timestamp --entitlements desktop/Ondera.entitlements \
      --sign "$APPLE_SIGNING_IDENTITY" "$target"
  done
  codesign --verify --deep --strict "$bundle"
  if [ -n "${APPLE_API_KEY_PATH:-}" ]; then
    # Notarize with an App Store Connect API key, then staple the ticket so the first launch
    # passes Gatekeeper offline.
    submission="dist/Ondera-notarize-$arch.zip"
    rm -f "$submission"
    ditto -c -k --keepParent "$bundle" "$submission"
    result=$(xcrun notarytool submit "$submission" --key "$APPLE_API_KEY_PATH" \
      --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" --wait --output-format json)
    rm -f "$submission"
    echo "$result"
    status=$(printf '%s' "$result" | python3 -c 'import json, sys; print(json.load(sys.stdin).get("status", ""))')
    if [ "$status" != Accepted ]; then
      id=$(printf '%s' "$result" | python3 -c 'import json, sys; print(json.load(sys.stdin).get("id", ""))')
      test -n "$id" && xcrun notarytool log "$id" --key "$APPLE_API_KEY_PATH" \
        --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" || true
      echo "Notarization was not accepted: $status" >&2
      exit 1
    fi
    xcrun stapler staple "$bundle"
    xcrun stapler validate "$bundle"
    spctl --assess --type execute --verbose=2 "$bundle"
  fi
else
  # Ad-hoc signing for local testing and self-updates when no Developer ID is configured.
  codesign --force --deep --sign - "$bundle"
  codesign --verify --deep --strict "$bundle"
fi
rm -f "dist/Ondera-macos-$arch.zip"
ditto -c -k --sequesterRsrc --keepParent "$bundle" "dist/Ondera-macos-$arch.zip"
echo "Built $bundle ($version, $arch) and dist/Ondera-macos-$arch.zip"
