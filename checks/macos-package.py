#!/usr/bin/env python3
"""Verify and launch a relocated PixelCraft Mac distribution."""

import argparse
import hashlib
import os
from pathlib import Path
import plistlib
import re
import struct
import subprocess
import sys


def run(args, **kwargs):
    return subprocess.run(args, check=True, timeout=90, **kwargs)


def verify_snapshot(shot):
    details = run(
        ["sips", "-g", "pixelWidth", "-g", "pixelHeight", str(shot)],
        capture_output=True,
        text=True,
    ).stdout
    match_width = re.search(r"pixelWidth:\s*(\d+)", details)
    match_height = re.search(r"pixelHeight:\s*(\d+)", details)
    if not match_width or not match_height or int(match_width[1]) < 800 or int(match_height[1]) < 600:
        raise ValueError("Graphical startup did not produce an editor-sized screenshot")
    preview = shot.with_suffix(".bmp")
    run(["sips", "-z", "120", "160", "-s", "format", "bmp", str(shot), "--out", str(preview)],
        stdout=subprocess.DEVNULL)
    data = preview.read_bytes()
    if len(data) < 54 or data[:2] != b"BM":
        raise ValueError("Cannot decode the sampled startup screenshot")
    offset = struct.unpack_from("<I", data, 10)[0]
    width, height, _, bits = struct.unpack_from("<iiHH", data, 18)
    if width != 160 or abs(height) != 120 or bits not in (24, 32):
        raise ValueError(f"Unexpected sampled screenshot format: {width}x{height}, {bits} bpp")
    size = bits // 8
    stride = (width * size + 3) & ~3
    if len(data) < offset + stride * abs(height):
        raise ValueError("Startup screenshot pixel data is truncated")
    colors = {
        data[start:start + 3]
        for row in range(abs(height))
        for start in range(offset + row * stride, offset + row * stride + width * size, size)
    }
    if len(colors) < 16:
        raise ValueError("Startup screenshot has too little rendered content")


def smoke(archive, output, arch):
    if sys.platform != "darwin":
        raise ValueError("Mac package smoke tests require macOS")
    sha256 = hashlib.sha256()
    with archive.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            sha256.update(block)
    checksum = archive.with_suffix(archive.suffix + ".sha256").read_text().split()
    if checksum != [sha256.hexdigest(), archive.name]:
        raise ValueError("Mac archive checksum mismatch")
    output.mkdir(parents=True, exist_ok=True)
    relocation = output / "Relocated Mac Apps"
    relocation.mkdir()
    run(["ditto", "-x", "-k", str(archive), str(relocation)])
    app = relocation / "PixelCraft.app"
    run(["codesign", "--verify", "--strict", str(app)])
    with (app / "Contents" / "Info.plist").open("rb") as stream:
        manifest = plistlib.load(stream)
    if manifest["CFBundlePackageType"] != "APPL" or manifest["CFBundleExecutable"] != "pixelcraft":
        raise ValueError("Invalid Mac application manifest")
    executable = app / "Contents" / "MacOS" / "pixelcraft"
    actual_arch = run(["lipo", "-archs", str(executable)], capture_output=True, text=True).stdout.strip()
    if actual_arch != arch:
        raise ValueError(f"Expected {arch}, found {actual_arch}")
    linked = run(["otool", "-L", str(executable)], capture_output=True, text=True).stdout.splitlines()[1:]
    library_paths = [line.strip().split(" (compatibility", 1)[0] for line in linked]
    if any(not path.startswith(("/usr/lib/", "/System/Library/")) for path in library_paths):
        raise ValueError(f"Mac package links a non-system runtime: {library_paths}")
    env = {key: value for key, value in os.environ.items() if not key.startswith("DYLD_")}
    env.update(
        PATH="/usr/bin:/bin:/usr/sbin:/sbin",
        PIXELCRAFT_HOME=str(output / "User Data"),
        XDG_CONFIG_HOME=str(output / "Config"),
    )
    commands = [
        ("standalone", ["--standalone-check", str(output / "standalone")]),
        ("startup", ["--screenshot", str(output / "startup.png")]),
    ]
    for name, args in commands:
        with (output / f"{name}.log").open("wb") as log:
            run([str(executable), *args], env=env, cwd=relocation, stdout=log, stderr=subprocess.STDOUT)
    verify_snapshot(output / "startup.png")
    print(f"macOS {arch} package and graphical startup passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("arch", choices=("arm64", "x86_64"))
    args = parser.parse_args()
    smoke(args.archive.resolve(strict=True), args.output.resolve(), args.arch)


if __name__ == "__main__":
    main()
