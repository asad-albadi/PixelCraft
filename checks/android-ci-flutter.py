#!/usr/bin/env python3
"""Install the pinned Linux x64 Flutter SDK, checking its published SHA-256."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tempfile
from urllib.request import urlopen

VERSION = "3.47.9"
DART_VERSION = "3.13.7"
ORIGIN = "https://storage.googleapis.com/flutter_infra_release/flutter"


def main():
    destination = Path(sys.argv[1]).resolve()
    destination.mkdir(parents=True, exist_ok=True)
    with urlopen(f"{ORIGIN}/releases/releases_linux.json", timeout=120) as response:
        releases = json.load(response)["releases"]
    matches = [r for r in releases if r["version"] == VERSION and r.get("dart_sdk_arch", "x64") == "x64"]
    if not matches:
        raise SystemExit(f"Official Linux x64 Flutter {VERSION} archive not found")
    release = matches[0]
    archive = PurePosixPath(release["archive"])
    if archive.is_absolute() or ".." in archive.parts or not str(archive).endswith(".tar.xz"):
        raise SystemExit("Invalid SDK archive path")
    expected = release["sha256"]
    with tempfile.TemporaryDirectory(prefix="pixelcraft-flutter-") as temporary:
        download = Path(temporary) / "flutter.tar.xz"
        digest = hashlib.sha256()
        with urlopen(f"{ORIGIN}/{archive}", timeout=120) as response, download.open("wb") as output:
            while chunk := response.read(1024 * 1024):
                digest.update(chunk)
                output.write(chunk)
        if digest.hexdigest() != expected:
            raise SystemExit("Flutter SDK SHA-256 mismatch")
        print(f"Flutter {VERSION} archive SHA-256 verified: {expected}", flush=True)
        subprocess.run(["tar", "-xf", str(download), "-C", str(destination)], check=True)
    sdk = destination / "flutter" / "bin"
    version = json.loads(subprocess.check_output([str(sdk / "flutter"), "--version", "--machine"], text=True))
    if version["frameworkVersion"] != VERSION or version["dartSdkVersion"].split()[0] != DART_VERSION:
        raise SystemExit(f"Unexpected SDK version: {version}")
    with open(os.environ["GITHUB_PATH"], "a") as output:
        output.write(f"{sdk}\n")
    subprocess.run([str(sdk / "flutter"), "precache", "--android"], check=True)


if __name__ == "__main__":
    main()
