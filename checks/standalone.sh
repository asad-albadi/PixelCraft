#!/bin/sh
# Run after cargo build --offline. This only restricts the test filesystem.
set -eu
app_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output_root=${1:-$(mktemp -d /tmp/pixelcraft-standalone.XXXXXX)}
mkdir -p "$output_root"
bwrap --unshare-user --unshare-pid --unshare-ipc --unshare-uts --die-with-parent \
  --ro-bind /usr/lib/libc.so.6 /usr/lib/libc.so.6 \
  --ro-bind /usr/lib/libm.so.6 /usr/lib/libm.so.6 \
  --ro-bind /usr/lib/libgcc_s.so.1 /usr/lib/libgcc_s.so.1 \
  --ro-bind /usr/lib64/ld-linux-x86-64.so.2 /usr/lib64/ld-linux-x86-64.so.2 \
  --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
  --dir /home/user --tmpfs /tmp --dev /dev --proc /proc \
  --bind "$output_root" /work --ro-bind "$app_root/target/debug/pixelcraft" /pixelcraft \
  --clearenv --setenv HOME /home/user --setenv PATH /nonexistent \
  --setenv PIXELCRAFT_HOME /work/data --setenv XDG_CONFIG_HOME /work/config --setenv XDG_DATA_HOME /work/data \
  /pixelcraft --standalone-check /work/results
printf 'Verified standalone artifacts: %s\n' "$output_root"
