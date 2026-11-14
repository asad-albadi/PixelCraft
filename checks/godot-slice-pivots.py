#!/usr/bin/env python3
"""Render PixelCraft's exported slice pivots in Godot 4 with a graphical display."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pixelcraft", default="target/debug/pixelcraft")
    parser.add_argument("--godot", default="godot")
    parser.add_argument("--display-driver", help="Godot display driver, such as wayland or x11")
    args = parser.parse_args()
    pixelcraft = shutil.which(args.pixelcraft)
    godot = shutil.which(args.godot)
    if not pixelcraft or not godot:
        parser.error("Build PixelCraft and provide the paths to its binary and Godot 4")
    pixelcraft, godot = map(Path.resolve, map(Path, (pixelcraft, godot)))

    def cli(operation, **arguments):
        result = subprocess.run(
            [pixelcraft, "--cli", operation, json.dumps(arguments)],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode:
            raise SystemExit(f"PixelCraft {operation} failed\n{result.stderr}{result.stdout}")
        return json.loads(result.stdout)

    slice_data = {
        "name": "anchor",
        "keys": [
            {"frame": 0, "bounds": [2, 1, 5, 5], "pivot": [2, 2]},
            {"frame": 1, "bounds": [6, 2, 5, 6], "pivot": [1, 3]},
        ],
    }
    with tempfile.TemporaryDirectory(prefix="pixelcraft-godot-slices-") as temporary:
        project = Path(temporary)
        (project / "project.godot").write_text(
            'config_version=5\n[application]\nconfig/name="PixelCraft Slice Pivot Check"\n',
            encoding="utf-8",
        )
        shutil.copyfile(Path(__file__).with_suffix(".gd"), project / "check.gd")

        blank = project / "blank.pixel"
        aseprite = project / "authored.aseprite"
        reopened = project / "reopened.pixel"
        revision = cli("create", output=str(blank), width=12, height=9)["revision"]
        commands = [
            {"op": "add-frame", "frame": 0},
            {"op": "add-frame", "frame": 1},
        ]
        frames = [
            (3, 2, 5, 4, [255, 0, 0, 255], 4, 3),
            (6, 4, 9, 6, [0, 255, 0, 255], 7, 5),
            (7, 3, 8, 5, [0, 0, 255, 255], 7, 5),
        ]
        for frame, (x, y, x2, y2, ink, anchor_x, anchor_y) in enumerate(frames):
            commands.extend(
                [
                    {
                        "op": "shape", "layer": 0, "frame": frame, "kind": "rectangle",
                        "filled": True, "x": x, "y": y, "x2": x2, "y2": y2, "color": ink,
                    },
                    {
                        "op": "shape", "layer": 0, "frame": frame, "kind": "line",
                        "color": [255] * 4, "x": anchor_x, "y": anchor_y,
                        "x2": anchor_x, "y2": anchor_y,
                    },
                ]
            )
        commands.append({"op": "set-slice", "slice": slice_data})
        cli("commands", input=str(blank), output=str(aseprite),
            revision=revision, commands=commands)
        cli("export", input=str(aseprite), output=str(reopened))
        keys = cli("inspect", input=str(reopened))["slices"][0]["keys"]
        if [{k: key[k] for k in ("frame", "bounds", "pivot")} for key in keys] != slice_data["keys"]:
            raise SystemExit("Slice keys changed after reopening ASE and native files")
        cli(
            "export",
            input=str(reopened),
            output=str(project / "atlas.png"),
            format="sheet",
            options={"preset": "Godot", "trim": True, "padding": 3, "scale": 2, "columns": 2},
        )
        subprocess.run(
            [godot, "--headless", "--path", project, "--import"],
            check=True,
            timeout=60,
        )
        launch = [godot, "--path", project, "--rendering-method", "gl_compatibility",
                  "--audio-driver", "Dummy", "--script", "res://check.gd"]
        if args.display_driver:
            launch.extend(["--display-driver", args.display_driver])
        subprocess.run(launch, check=True, timeout=60)


if __name__ == "__main__":
    main()
