#!/usr/bin/env bash
# Run on an existing X11 or Wayland display; --screenshot closes after rendering.
set -euo pipefail
if [[ $# != 2 ]]; then
  echo "Usage: $0 APPIMAGE OUTPUT_DIRECTORY" >&2
  exit 2
fi
app=$(realpath -- "$1")
mkdir -p -- "$2"
output=$(realpath -- "$2")
shot="$output/startup.png"
rm -f -- "$shot"
export APPIMAGE_EXTRACT_AND_RUN=1
export LIBGL_ALWAYS_SOFTWARE=1
export PIXELCRAFT_HOME="$output/data"
export XDG_CONFIG_HOME="$output/config"
if ! timeout --kill-after=5s 60s "$app" --screenshot "$shot" >"$output/startup.log" 2>&1; then
  cat "$output/startup.log" >&2
  echo "Graphical startup failed or timed out." >&2
  exit 1
fi
cat "$output/startup.log"
if [[ ! -s "$shot" ]]; then
  echo "Graphical startup did not produce a screenshot." >&2
  exit 1
fi
python3 - "$shot" <<'PY'
import sys
from PIL import Image

with Image.open(sys.argv[1]) as image:
    image.load()
    if image.format != "PNG" or image.width < 800 or image.height < 600:
        raise SystemExit("Startup did not produce an editor-sized PNG")
    colors = image.convert("RGB").resize((160, 120)).getcolors(160 * 120)
    if len(colors) < 16:
        raise SystemExit("Startup image is blank or has too little rendered content")
    print(f"Graphical startup passed: rendered {image.width}x{image.height} PNG")
PY
