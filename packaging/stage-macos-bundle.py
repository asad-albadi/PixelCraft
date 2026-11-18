#!/usr/bin/env python3
"""Stage the executable, metadata, icon, and notices for the Mac release bundle."""

import argparse
from pathlib import Path
import plistlib
import re
import shutil

REPO = Path(__file__).resolve().parent.parent


def stage(binary, icon, app, version, minimum, webp):
    short_version = re.fullmatch(
        r"([0-9]+\.[0-9]+\.[0-9]+)(?:[-+][0-9A-Za-z.+-]+)?", version
    )
    if not short_version or not re.fullmatch(r"[0-9]+\.[0-9]+(?:\.[0-9]+)?", minimum):
        raise ValueError("Invalid bundle or deployment version")
    executable = app / "Contents" / "MacOS" / "pixelcraft"
    resources = app / "Contents" / "Resources"
    files = {
        executable: binary,
        resources / "pixelcraft.icns": icon,
        resources / "LICENSE": REPO / "LICENSE",
        resources / "FONT-LICENSE.txt": REPO / "assets/fonts/LICENSE",
        resources / "LUA-LICENSE.txt": REPO / "runtime/LUA-LICENSE.txt",
    }
    for name in ("WEBP-COPYING.txt", "WEBP-PATENTS.txt", "WEBP-AUTHORS.txt"):
        files[resources / name] = webp / name
    for source in files.values():
        if not source.is_file() or source.stat().st_size == 0:
            raise ValueError(f"Missing bundle input: {source}")
    executable.parent.mkdir(parents=True)
    resources.mkdir()
    for target, source in files.items():
        shutil.copyfile(source, target)
        target.chmod(0o755 if target == executable else 0o644)

    manifest = {
        "CFBundleDisplayName": "PixelCraft",
        "CFBundleName": "PixelCraft",
        "CFBundleExecutable": executable.name,
        "CFBundleIconFile": "pixelcraft.icns",
        "CFBundleIdentifier": "io.github.asad-albadi.PixelCraft",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": short_version[1],
        "CFBundleVersion": short_version[1],
        "LSMinimumSystemVersion": minimum,
        "NSHighResolutionCapable": True,
    }
    with (app / "Contents" / "Info.plist").open("wb") as output:
        plistlib.dump(manifest, output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("icon", type=Path)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("version")
    parser.add_argument("minimum")
    parser.add_argument("webp", type=Path)
    args = parser.parse_args()
    stage(args.binary, args.icon, args.bundle, args.version, args.minimum, args.webp)


if __name__ == "__main__":
    main()
