#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 3 ]]; then
  echo "Usage: $0 TARBALL DEB OUTPUT_DIRECTORY" >&2
  exit 2
fi
tarball=$(realpath -- "$1")
deb=$(realpath -- "$2")
mkdir -p -- "$3"
output=$(realpath -- "$3")
repo=$(realpath -- "$(dirname -- "$0")/..")
fixture="$repo/tests/fixtures/workflows/animation.webp"
verify_checksum() {
  local file=$1
  (cd "$(dirname -- "$file")" && sha256sum --check --strict -- "$(basename -- "$file").sha256")
}
verify_checksum "$tarball"
verify_checksum "$deb"

smoke() {
  local app=$1 work=$2
  mkdir -p -- "$work"
  export PIXELCRAFT_HOME="$work/data"
  export XDG_CONFIG_HOME="$work/config"
  local -a environment=(env -u APPIMAGE -u APPDIR -u LD_LIBRARY_PATH -u WAYLAND_DISPLAY -u WAYLAND_SOCKET)
  (
    cd "$work"
    "${environment[@]}" timeout --kill-after=5s 60s "$app" --standalone-check "$work/results"
    "${environment[@]}" timeout --kill-after=5s 60s "$app" --cli inspect --input "$fixture" > "$work/webp.json"
    "${environment[@]}" timeout --kill-after=5s 60s "$app" --cli render --input "$fixture" \
      --output "$work/webp-frame.png" --frame 1
  )
  python3 - "$work" <<'PY'
import json
from pathlib import Path
import sys

from PIL import Image

directory = Path(sys.argv[1])
report = json.loads((directory / "webp.json").read_text())
assert (report["width"], report["height"], report["frames"]) == (3, 2, [70, 230]), report
with Image.open(directory / "webp-frame.png") as image:
    pixels = image.convert("RGBA")
    assert pixels.getpixel((2, 1)) == (0, 255, 0, 255)
    assert pixels.getpixel((0, 0)) == (0, 0, 0, 0)
PY
  "${environment[@]}" xvfb-run -a bash "$repo/checks/graphical-startup.sh" "$app" "$work/startup"
}

extract="$output/Relocated tarball"
mkdir -p -- "$extract"
tar -xf "$tarball" -C "$extract"
root=$(basename -- "$tarball" .tar.gz)
test -x "$extract/$root/pixelcraft"
test -s "$extract/$root/usr/share/doc/pixelcraft/LICENSE"
smoke "$extract/$root/pixelcraft" "$output/tarball"

[[ $(dpkg-deb --field "$deb" Package) == pixelcraft ]]
[[ $(dpkg-deb --field "$deb" Architecture) == amd64 ]]
dpkg-deb --field "$deb" Depends | grep -F 'libwebpdemux2'
installed() {
  [[ $(dpkg-query -W -f='${db:Status-Status}' pixelcraft 2>/dev/null || true) == installed ]]
}
if installed || [[ -e /usr/bin/pixelcraft ]]; then
  echo "Run the Debian installation check on a runner without PixelCraft installed" >&2
  exit 1
fi
cleanup() {
  if installed; then
    sudo dpkg --remove pixelcraft
  fi
}
trap cleanup EXIT
sudo apt-get install -y --no-install-recommends "$deb"
dpkg-query -L pixelcraft | grep -Fx '/usr/share/applications/pixelcraft.desktop'
dpkg-query -L pixelcraft | grep -Fx '/usr/share/icons/hicolor/256x256/apps/pixelcraft.png'
desktop-file-validate /usr/share/applications/pixelcraft.desktop
smoke /usr/bin/pixelcraft "$output/debian"
cleanup
[[ ! -e /usr/bin/pixelcraft ]]
test -s "$output/debian/results/standalone.pixel"
