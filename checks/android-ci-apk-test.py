#!/usr/bin/env python3
"""Exercise APK verification without building Flutter or running an emulator."""
import contextlib
import importlib.util
import io
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("android_ci_apk", Path(__file__).with_name("android-ci-apk.py"))
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)


def elf(machine, alignment=16384):
    header = struct.pack("<16sHHIQQQIHHHHHH", b"\x7fELF\x02\x01" + b"\0" * 10,
                         3, machine, 1, 0, 64, 0, 0, 64, 56, 1, 0, 0, 0)
    return header + struct.pack("<IIQQQQQQ", 1, 5, 0, 0, 0, 120, 120, alignment)


class ApkTests(unittest.TestCase):
    def test_accepts_both_aligned_abis(self):
        self.check_apk()

    def test_rejects_missing_engine(self):
        with self.assertRaisesRegex(ValueError, "Missing"):
            self.check_apk(missing=True)

    def test_rejects_compressed_libraries(self):
        with self.assertRaisesRegex(ValueError, "uncompressed"):
            self.check_apk(compression=zipfile.ZIP_DEFLATED)

    def test_rejects_wrong_architecture(self):
        with self.assertRaisesRegex(ValueError, "machine"):
            verifier.verify_elf(elf(62), 183, "wrong")

    def test_rejects_4k_alignment(self):
        with self.assertRaisesRegex(ValueError, "16 KiB"):
            verifier.verify_elf(elf(183, 4096), 183, "4k")

    def test_rejects_truncated_headers(self):
        with self.assertRaisesRegex(ValueError, "program header"):
            verifier.verify_elf(elf(183)[:100], 183, "short")

    def check_apk(self, missing=False, compression=zipfile.ZIP_STORED):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test.apk"
            with zipfile.ZipFile(path, "w", compression=compression) as apk:
                for abi, machine in verifier.MACHINES.items():
                    for library in ("libpixelcraft_ffi.so", "libflutter.so"):
                        if missing and library == "libpixelcraft_ffi.so":
                            continue
                        apk.writestr(f"lib/{abi}/{library}", elf(machine))
            with contextlib.redirect_stdout(io.StringIO()):
                verifier.verify_apk(path)


if __name__ == "__main__":
    unittest.main()
