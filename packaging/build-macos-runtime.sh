#!/usr/bin/env bash
set -euo pipefail
arch=${1:-$(uname -m)}
case "$arch" in
  arm64|x86_64) ;;
  *) echo "Expected arm64 or x86_64" >&2; exit 2 ;;
esac
if [[ $(uname -s) != Darwin ]]; then
  echo "Build the macOS runtime on macOS" >&2
  exit 2
fi
export MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-12.0}
repo=$(cd "$(dirname "$0")/.." && pwd -P)
output="$repo/runtime/macos/$arch"
parent=$(cd "${TMPDIR:-/tmp}" && pwd -P)
work=$(mktemp -d "$parent/pixelcraft-macos-runtime.XXXXXXXX")
[[ $work == "$parent"/pixelcraft-macos-runtime.* ]]
trap 'rm -rf -- "$work"' EXIT

webp="$work/webp.tar.gz"
curl --fail --location --retry 3 \
  'https://storage.googleapis.com/downloads.webmproject.org/releases/webp/libwebp-1.6.0.tar.gz' \
  --output "$webp"
printf '%s  %s\n' 'e4ab7009bf0629fd11982d4c2aa83964cf244cffba7347ecd39019a9e38c4564' "$webp" |
  shasum -a 256 --check -
tar -xzf "$webp" -C "$work"
cmake -S "$work/libwebp-1.6.0" -B "$work/webp-build" \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_ARCHITECTURES="$arch" \
  -DCMAKE_OSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET" \
  -DBUILD_SHARED_LIBS=OFF -DWEBP_LINK_STATIC=ON \
  -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF -DWEBP_BUILD_DWEBP=OFF \
  -DWEBP_BUILD_GIF2WEBP=OFF -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF \
  -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_LIBWEBPMUX=OFF \
  -DWEBP_BUILD_WEBPMUX=OFF -DWEBP_BUILD_EXTRAS=OFF
cmake --build "$work/webp-build" --config Release --target webpdemux --parallel 3
mkdir -p "$output"
for name in webpdemux webp sharpyuv; do
  archive=$(find "$work/webp-build" -type f -name "lib$name.a" -print -quit)
  [[ -n "$archive" && $(lipo -archs "$archive") == "$arch" ]]
  install -m 644 "$archive" "$output/lib$name.a"
done
for notice in COPYING PATENTS AUTHORS; do
  install -m 644 "$work/libwebp-1.6.0/$notice" "$output/WEBP-$notice.txt"
done
