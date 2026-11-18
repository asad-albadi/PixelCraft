#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 3 ]]; then
  echo "Usage: $0 APPDIR OUTPUT_DIRECTORY ASSET_VERSION" >&2
  exit 2
fi
appdir=$(realpath -- "$1")
mkdir -p -- "$2"
output=$(realpath -- "$2")
version=$3
[[ $version =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || { echo "Invalid asset version" >&2; exit 2; }
[[ -x "$appdir/usr/bin/pixelcraft" ]] || { echo "AppDir is missing its executable" >&2; exit 1; }
[[ -e "$appdir/usr/lib/libwebpdemux.so.2" ]] || { echo "AppDir is missing the WebP runtime" >&2; exit 1; }

temporary_parent=$(realpath -- "${TMPDIR:-/tmp}")
stage=$(mktemp -d "$temporary_parent/pixelcraft-tarball.XXXXXXXX")
[[ $stage == "$temporary_parent"/pixelcraft-tarball.* ]]
trap 'rm -rf -- "$stage"' EXIT
name="PixelCraft-$version-linux-x86_64"
mkdir -p -- "$stage/$name"
cp -a -- "$appdir/usr" "$stage/$name/usr"
cat > "$stage/$name/pixelcraft" <<'LAUNCHER'
#!/bin/sh
set -eu
here=$(dirname -- "$(realpath -- "$0")")
export LD_LIBRARY_PATH="$here/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$here/usr/bin/pixelcraft" "$@"
LAUNCHER
chmod 755 "$stage/$name/pixelcraft"

tar --sort=name --owner=0 --group=0 --numeric-owner -C "$stage" -cf - "$name" |
  gzip -n > "$stage/$name.tar.gz"
mv -- "$stage/$name.tar.gz" "$output/$name.tar.gz"
(cd "$output" && sha256sum -- "$name.tar.gz" > "$name.tar.gz.sha256")
