#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 4 ]]; then
  echo "Usage: $0 RELEASE_BINARY OUTPUT_DIRECTORY ASSET_VERSION CARGO_VERSION" >&2
  exit 2
fi
binary=$(realpath -- "$1")
mkdir -p -- "$2"
output=$(realpath -- "$2")
asset_version=$3
package_version=${4/-/\~}
[[ $asset_version =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || { echo "Invalid asset version" >&2; exit 2; }
dpkg --validate-version "$package_version"
[[ $(dpkg --print-architecture) == amd64 ]] || { echo "This recipe builds amd64 packages" >&2; exit 1; }
LC_ALL=C readelf -h "$binary" | grep -F 'Advanced Micro Devices X86-64' >/dev/null
desktop-file-validate packaging/pixelcraft.desktop

temporary_parent=$(realpath -- "${TMPDIR:-/tmp}")
stage=$(mktemp -d "$temporary_parent/pixelcraft-deb.XXXXXXXX")
[[ $stage == "$temporary_parent"/pixelcraft-deb.* ]]
trap 'rm -rf -- "$stage"' EXIT
mkdir -p -- "$stage/debian" "$stage/package/DEBIAN"
cat > "$stage/debian/control" <<'CONTROL'
Source: pixelcraft
Section: graphics
Priority: optional
Maintainer: PixelCraft maintainers <asad-albadi@users.noreply.github.com>

Package: pixelcraft
Architecture: amd64
Description: Pixel art, sprites and animation editor
CONTROL
linked=$(cd "$stage" && dpkg-shlibdeps -O -e"$binary")
if [[ $linked != shlibs:Depends=?* || $linked == *$'\n'* ]]; then
  echo "Could not derive the ELF library dependencies" >&2
  exit 1
fi
linked=${linked#shlibs:Depends=}

package="$stage/package"
install -D -m 755 "$binary" "$package/usr/bin/pixelcraft"
install -D -m 644 packaging/pixelcraft.desktop "$package/usr/share/applications/pixelcraft.desktop"
install -D -m 644 packaging/pixelcraft.png "$package/usr/share/icons/hicolor/256x256/apps/pixelcraft.png"
install -D -m 644 LICENSE "$package/usr/share/doc/pixelcraft/copyright"
install -D -m 644 assets/fonts/LICENSE "$package/usr/share/doc/pixelcraft/FONT-LICENSE.txt"
install -D -m 644 runtime/LUA-LICENSE.txt "$package/usr/share/doc/pixelcraft/LUA-LICENSE.txt"
size=$(du -sk -- "$package/usr" | cut -f1)
# Libraries opened at runtime are not visible to dpkg-shlibdeps.
runtime='libwebpdemux2, libgl1, libegl1, libx11-6, libxcursor1, libxi6, libxrandr2, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libwayland-egl1'
cat > "$package/DEBIAN/control" <<CONTROL
Package: pixelcraft
Version: $package_version
Section: graphics
Priority: optional
Architecture: amd64
Maintainer: PixelCraft maintainers <asad-albadi@users.noreply.github.com>
Installed-Size: $size
Depends: $linked, $runtime
Recommends: libgl1-mesa-dri, xdg-desktop-portal
Suggests: ffmpeg, wl-clipboard
Homepage: https://github.com/asad-albadi/PixelCraft
Description: Pixel art, sprites and animation editor
 Desktop editor for layered sprites, animation and game asset export.
CONTROL

name="PixelCraft-$asset_version-linux-amd64.deb"
dpkg-deb --root-owner-group --build "$package" "$stage/$name"
mv -- "$stage/$name" "$output/$name"
(cd "$output" && sha256sum -- "$name" > "$name.sha256")
