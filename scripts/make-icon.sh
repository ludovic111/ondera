#!/bin/sh
# Render ryolune's app icon from its source, desktop/icons/ryolune.svg (the lsuite icon
# template in teal), into desktop/assets/ryolune.icns and desktop/icons/icon.png.
# macOS only: QuickLook rasterizes the SVG, sips resizes, iconutil packs the .icns.
# desktop/icons/icon.ico (Windows) is not remade here; any SVG-to-ICO tool will do.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
qlmanage -t -s 1024 -o "$work" "$root/desktop/icons/ryolune.svg" >/dev/null
master="$work/ryolune.svg.png"
set_dir="$work/ryolune.iconset"
mkdir "$set_dir"
for size in 16 32 128 256 512; do
  sips -z $size $size "$master" --out "$set_dir/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double "$master" --out "$set_dir/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$set_dir" -o "$root/desktop/assets/ryolune.icns"
sips -z 512 512 "$master" --out "$root/desktop/icons/icon.png" >/dev/null
echo "Wrote desktop/assets/ryolune.icns and desktop/icons/icon.png"
