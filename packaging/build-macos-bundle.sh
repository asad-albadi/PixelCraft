#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 5 ]]; then
  echo "Usage: $0 BINARY OUTPUT_DIRECTORY ASSET_VERSION CARGO_VERSION ARCH" >&2
  exit 2
fi
repo=$(cd "$(dirname "$0")/.." && pwd -P)
binary=$1
mkdir -p "$2"
output=$(cd "$2" && pwd -P)
version=$3
cargo_version=$4
arch=$5
[[ $version =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || { echo "Invalid asset version" >&2; exit 2; }
case "$arch" in arm64|x86_64) ;; *) exit 2 ;; esac
[[ $(lipo -archs "$binary") == "$arch" ]]
parent=$(cd "${TMPDIR:-/tmp}" && pwd -P)
work=$(mktemp -d "$parent/pixelcraft-macos-package.XXXXXXXX")
[[ $work == "$parent"/pixelcraft-macos-package.* ]]
trap 'rm -rf -- "$work"' EXIT

iconset="$work/pixelcraft.iconset"
mkdir "$iconset"
for scale in 16 32 128 256 512; do
  sips -z "$scale" "$scale" "$repo/packaging/pixelcraft.png" \
    --out "$iconset/icon_${scale}x${scale}.png" >/dev/null
  large=$((scale * 2))
  sips -z "$large" "$large" "$repo/packaging/pixelcraft.png" \
    --out "$iconset/icon_${scale}x${scale}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$work/pixelcraft.icns"
app="$work/PixelCraft.app"
python3 "$repo/packaging/stage-macos-bundle.py" "$binary" "$work/pixelcraft.icns" \
  "$app" "$cargo_version" "${MACOSX_DEPLOYMENT_TARGET:-12.0}" "$repo/runtime/macos/$arch"
plutil -lint "$app/Contents/Info.plist"
codesign --force --sign - "$app"
codesign --verify --strict "$app"

name="PixelCraft-$version-macos-$arch.zip"
ditto -c -k --keepParent "$app" "$work/$name"
mv -- "$work/$name" "$output/$name"
(cd "$output" && shasum -a 256 "$name" > "$name.sha256")
