#!/usr/bin/env python3
"""Measure real Wayland compositor presentations and Linux DRM memory during playback."""

import argparse
import json
import math
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time


PRESENTED = re.compile(r"wp_presentation_feedback#\d+\.presented\(([^)]*)\)")
MEMORY = re.compile(r"^drm-memory-(vram|gtt):\s+(\d+) KiB$", re.MULTILINE)
CLIENT = re.compile(r"^drm-client-id:\s+(\d+)$", re.MULTILINE)


def sample_memory(pid):
    proc = Path(f"/proc/{pid}")
    status = (proc / "status").read_text()
    ram = int(re.search(r"^VmHWM:\s+(\d+)", status, re.MULTILINE)[1])
    clients = {}
    for fd in (proc / "fdinfo").iterdir():
        try:
            info = fd.read_text()
        except (FileNotFoundError, PermissionError):
            continue
        client = CLIENT.search(info)
        if client:
            clients[client[1]] = dict((kind, int(kib)) for kind, kib in MEMORY.findall(info))
    return [ram, *(sum(client.get(kind, 0) for client in clients.values())
                   if any(kind in client for client in clients.values()) else None
                   for kind in ("vram", "gtt"))]


def measure(binary, fixture, warmup, seconds, directory):
    data = directory / "data"
    config = directory / "config"
    temporary = directory / "tmp"
    temporary.mkdir()
    env = dict(os.environ, WAYLAND_DEBUG="1", WGPU_BACKEND="vulkan",
               PIXELCRAFT_HOME=str(data), XDG_CONFIG_HOME=str(config), TMPDIR=str(temporary))
    for name in ("DISPLAY", "PIXELCRAFT_OPENGL", "LIBGL_ALWAYS_SOFTWARE"):
        env.pop(name, None)
    log_path = directory / "wayland.log"
    with log_path.open("w") as log:
        launch = time.monotonic_ns()
        proc = subprocess.Popen([binary, "--open", fixture, "--play"], env=env, stderr=log)
        try:
            time.sleep(warmup)
            start = time.monotonic_ns()
            end = start + int(seconds * 1e9)
            peaks = [None, None, None]
            while time.monotonic_ns() < end:
                if proc.poll() is not None:
                    raise SystemExit(f"Editor exited: inspect {log_path}")
                for i, n in enumerate(sample_memory(proc.pid)):
                    if n is not None:
                        peaks[i] = max(peaks[i] or 0, n)
                time.sleep(0.05)
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
    if peaks[1] is None:
        raise SystemExit(f"No hardware VRAM accounting: inspect {log_path}")
    log = log_path.read_text()
    if ".clock_id(1)" not in log:
        raise SystemExit("The compositor did not report CLOCK_MONOTONIC")
    samples = []
    for match in PRESENTED.finditer(log):
        hi, lo, nsec, refresh, _, _, flags = map(int, match[1].split(", "))
        when = ((hi << 32) + lo) * 1_000_000_000 + nsec
        if flags & 0x2:  # wp_presentation: timestamp measured by display hardware
            samples.append((when, refresh))
    observed = sorted(set(when for when, _ in samples if start <= when <= end))
    if len(observed) < 2:
        raise SystemExit(f"Too few real presentations. Keep the window visible: {log_path}")
    gaps = sorted((b - a) / 1e6 for a, b in zip(observed, observed[1:]))
    return {
        "fixture": str(fixture),
        "first_window_present_ms": round((min(t for t, _ in samples) - launch) / 1e6, 2),
        "samples": len(observed),
        "display_refresh_ms": sorted({round(refresh / 1e6, 2) for _, refresh in samples}),
        "gap_p95_ms": round(gaps[math.ceil(len(gaps) * .95) - 1], 2),
        "gap_max_ms": round(gaps[-1], 2),
        "gaps_over_16_7_ms": sum(gap > 16.7 for gap in gaps),
        "peak_rss_kib": peaks[0],
        "peak_vram_kib": peaks[1],
        "peak_gtt_kib": peaks[2],
    }


def create_fixture(binary, output, kind):
    """Use ASE so both builds can open exactly the same layered input."""
    def cli(operation, **arguments):
        result = subprocess.run(
            [binary, "--cli", operation, json.dumps(arguments)], check=True,
            capture_output=True, text=True, timeout=60,
        )
        return json.loads(result.stdout)

    width, height, frames, layers = {
        "sparse": (512, 256, 120, 1), "dense": (1024, 768, 8, 12),
    }[kind]
    with tempfile.TemporaryDirectory(prefix="pixelcraft-playback-fixture-") as tmp:
        blank = Path(tmp) / "blank.pixel"
        revision = cli("create", width=width, height=height, output=str(blank))["revision"]
        commands = [{"op": "add-layer", "name": f"Layer {i + 1}"} for i in range(1, layers)]
        commands.extend({"op": "add-frame", "frame": i} for i in range(frames - 1))
        for frame in range(frames):
            commands.append({
                "op": "set-frame", "frame": frame, "duration": (17, 16, 17)[frame % 3],
            })
            for layer in range(layers):
                if layers == 1 and frame % 15:
                    continue
                x = frame % width if layers == 1 else 0
                commands.append({
                    "op": "shape", "layer": layer, "frame": frame, "kind": "rectangle",
                    "x": x, "y": 0, "x2": x if layers == 1 else width - 1,
                    "y2": 0 if layers == 1 else height - 1, "filled": True,
                    "color": [57 + frame % 128, 37 + layer * 11, 64 + frame % 80,
                              255 if layers == 1 else 128],
                })
        cli("commands", input=str(blank), output=str(output), revision=revision, commands=commands)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("--warmup", type=float, default=3)
    parser.add_argument("--seconds", type=float, default=8)
    parser.add_argument("--trace-dir", type=Path)
    parser.add_argument("--create-fixture", choices=("sparse", "dense"))
    args = parser.parse_args()
    if args.warmup < 0 or args.seconds < 1:
        parser.error("Use nonnegative warmup and at least one second of observation")
    binary = args.binary.resolve(strict=True)
    if args.create_fixture:
        create_fixture(binary, args.fixture.resolve(), args.create_fixture)
        return
    fixture = args.fixture.resolve(strict=True)
    if args.trace_dir:
        args.trace_dir.mkdir()
        print(json.dumps(measure(binary, fixture, args.warmup, args.seconds, args.trace_dir)))
    else:
        with tempfile.TemporaryDirectory(prefix="pixelcraft-presentation-") as tmp:
            print(json.dumps(measure(binary, fixture, args.warmup, args.seconds, Path(tmp))))


if __name__ == "__main__":
    main()
