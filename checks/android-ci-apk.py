#!/usr/bin/env python3
"""Check a universal APK contains both 64-bit engines with 16 KiB ELF LOADs.

ZIP page alignment and APK signatures are checked separately with Android SDK tools.
"""
import argparse
from pathlib import Path
import struct
import zipfile

MACHINES = {"arm64-v8a": 183, "x86_64": 62}
PAGE_SIZE = 16384


def verify_elf(data, machine, label):
    if len(data) < 64 or data[:6] != b"\x7fELF\x02\x01":
        raise ValueError(f"{label}: expected a little-endian 64-bit ELF")
    header = struct.unpack_from("<16sHHIQQQIHHHHHH", data)
    if header[2] != machine:
        raise ValueError(f"{label}: ELF machine does not match APK ABI directory")
    offset, entry_size, count = header[5], header[9], header[10]
    if not count or entry_size < 56 or offset + entry_size * count > len(data):
        raise ValueError(f"{label}: invalid ELF program header table")
    loads = 0
    for index in range(count):
        kind, _, file_offset, address, _, file_size, _, alignment = struct.unpack_from(
            "<IIQQQQQQ", data, offset + entry_size * index)
        if kind != 1:  # PT_LOAD
            continue
        loads += 1
        if (alignment < PAGE_SIZE or alignment & (alignment - 1)
                or file_offset % PAGE_SIZE != address % PAGE_SIZE
                or file_offset + file_size > len(data)):
            raise ValueError(f"{label}: LOAD segment is not valid for 16 KiB pages")
    if not loads:
        raise ValueError(f"{label}: no ELF LOAD segments")


def verify_apk(path):
    with zipfile.ZipFile(path) as apk:
        entries = apk.infolist()
        names = [entry.filename for entry in entries]
        if len(names) != len(set(names)):
            raise ValueError("Duplicate APK ZIP entries")
        expected = {f"lib/{abi}/{library}" for abi in MACHINES
                    for library in ("libpixelcraft_ffi.so", "libflutter.so")}
        if not expected <= set(names):
            raise ValueError(f"Missing universal APK native libraries: {sorted(expected - set(names))}")
        libraries = [entry for entry in entries if entry.filename.startswith("lib/") and entry.filename.endswith(".so")]
        for entry in libraries:
            parts = entry.filename.split("/")
            if len(parts) != 3 or parts[1] not in MACHINES:
                raise ValueError(f"Unexpected native ABI/path: {entry.filename}")
            if entry.compress_type != zipfile.ZIP_STORED:
                raise ValueError(f"{entry.filename}: native library must be uncompressed for page alignment")
            verify_elf(apk.read(entry), MACHINES[parts[1]], entry.filename)
        print(f"Verified {len(libraries)} native libraries: arm64-v8a + x86_64, ELF LOAD alignment >=16 KiB")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", type=Path)
    args = parser.parse_args()
    try:
        verify_apk(args.apk)
    except (OSError, ValueError, zipfile.BadZipFile, struct.error) as error:
        parser.exit(1, f"APK check failed: {error}\n")
