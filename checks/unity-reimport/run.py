#!/usr/bin/env python3
"""Verify same-path PixelCraft sprite and animation imports in Unity Editor."""

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path


UNITY_VERSION = "6000.3.28f1"
DURATIONS = [93, 147, 71, 129, 121, 203]
COLORS = [[245, 30, 20], [225, 85, 25], [50, 230, 45], [25, 10, 235],
          [180, 40, 200], [230, 40, 90]]


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def shape(frame, rgb, first=(2, 2), last=(4, 3), alpha=255):
    return {
        "op": "shape", "layer": 0, "frame": frame, "kind": "rectangle",
        "x": first[0], "y": first[1], "x2": last[0], "y2": last[1],
        "filled": True, "color": [*rgb, alpha],
    }


def fixture_operations(project, pixelcraft):
    fixture = project / "Fixture"
    fixture.mkdir()
    blank = fixture / "blank.pixel"
    initial = fixture / "initial.pixel"
    redraw = fixture / "redraw.pixel"
    insert = fixture / "insert.pixel"

    def cli(operation, **arguments):
        result = subprocess.run(
            [pixelcraft, "--cli", operation, json.dumps(arguments)],
            text=True, capture_output=True, timeout=45,
        )
        if result.returncode:
            raise RuntimeError(f"PixelCraft {operation} failed\n{result.stderr}{result.stdout}")
        return json.loads(result.stdout)

    def edit(source, output, commands):
        revision = cli("inspect", input=str(source))["revision"]
        return cli("commands", input=str(source), output=str(output),
                   revision=revision, commands=commands)

    def export(source):
        assets = project / "Assets"
        cli("export", input=str(source), output=str(assets / "hero.png"), format="sheet",
            options={"preset": "Unity", "trim": False, "padding": 0, "scale": 1, "columns": 4})
        direct = assets / "Direct"
        direct.mkdir(exist_ok=True)
        cli("export", input=str(source), output=str(direct / "hero.aseprite"))

    cli("create", width=8, height=6, output=str(blank))
    commands = [{"op": "add-frame", "frame": i} for i in range(5)]
    for i, duration in enumerate(DURATIONS):
        commands.extend([{"op": "set-frame", "frame": i, "duration": duration}, shape(i, COLORS[i])])
    for i, name in enumerate(("idle", "walk", "jump")):
        commands.append({"op": "add-tag", "tag": {
            "name": name, "from": i * 2, "to": i * 2 + 1, "direction": 0,
            "repeat": 0, "color": [100, 120, 140],
        }})
    edit(blank, initial, commands)
    export(initial)
    yield "first"

    edit(initial, redraw, [
        shape(3, [0, 0, 0], first=(0, 0), last=(7, 5), alpha=0),
        shape(3, [25, 180, 230], first=(1, 1), last=(6, 4)),
    ])
    export(redraw)
    yield "redraw"

    edit(redraw, insert, [
        {"op": "add-frame", "frame": 2},
        shape(3, [220, 180, 20]),
        {"op": "set-frame", "frame": 3, "duration": 111},
        {"op": "set-frame", "frame": 4, "duration": 173},
    ])
    export(insert)
    yield "insert"


def main():
    # Unity diagnostics may contain characters absent from legacy Windows code pages.
    sys.stdout.reconfigure(errors="backslashreplace")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pixelcraft", default="target/debug/pixelcraft")
    parser.add_argument("--unity", default="Unity")
    parser.add_argument("--editor-version", default=UNITY_VERSION)
    parser.add_argument("--aseprite-package", default="3.0.2",
                        help="Pinned com.unity.2d.aseprite package version")
    default_importer = Path(__file__).resolve().parents[2] / "integrations/unity/Editor/PixelCraftSpriteSheetImporter.cs"
    parser.add_argument("--importer", type=Path, default=default_importer,
                        help="Path to PixelCraft's Unity importer script")
    parser.add_argument("--project", type=Path, required=True,
                        help="A new or empty directory for the generated Unity project")
    parser.add_argument("--route", choices=("both", "sheet", "aseprite"), default="both")
    parser.add_argument("--fixtures-only", action="store_true",
                        help="Generate the three successive exports and skip Unity")
    args = parser.parse_args()
    pixelcraft = shutil.which(args.pixelcraft)
    unity = shutil.which(args.unity)
    if not pixelcraft or (not args.fixtures_only and not unity):
        parser.error("Provide a built PixelCraft binary and an activated Unity Editor")
    if not args.fixtures_only and not args.importer.is_file():
        parser.error("Supply the PixelCraft Unity importer using --importer")
    project = args.project.resolve()
    if project.exists() and any(project.iterdir()):
        parser.error("Choose a new or empty --project directory")
    assets = project / "Assets"
    editor = assets / "Editor"
    editor.mkdir(parents=True)
    write(project / "ProjectSettings/ProjectVersion.txt", f"m_EditorVersion: {args.editor_version}\n")
    write(project / "Packages/manifest.json", json.dumps({
        "dependencies": {
            "com.unity.2d.aseprite": args.aseprite_package,
            "com.unity.modules.animation": "1.0.0",
            "com.unity.modules.imageconversion": "1.0.0",
        }
    }, indent=2) + "\n")
    if not args.fixtures_only:
        shutil.copyfile(args.importer, editor / "SpriteSheetImporter.cs")
        shutil.copyfile(Path(__file__).with_name("UnityReimportCheck.cs"), editor / "UnityReimportCheck.cs")
    routes = ["sheet", "aseprite"] if args.route == "both" else [args.route]
    failed = {}
    for phase in fixture_operations(project, str(Path(pixelcraft).resolve())):
        if args.fixtures_only:
            output = project / "Fixture" / phase
            output.mkdir()
            for source in (*assets.glob("hero.*"), assets / "Direct/hero.aseprite"):
                shutil.copyfile(source, output / source.name)
            print(f"Exported {phase} fixture to {output}", flush=True)
            continue
        for route in routes:
            if route in failed:
                continue
            log = project / f"unity-{route}-{phase}.log"
            try:
                result = subprocess.run([
                    str(Path(unity).resolve()), "-batchmode", "-nographics", "-quit",
                    "-projectPath", str(project), "-executeMethod", "PixelCraft.Checks.UnityReimportCheck.Run",
                    "-pixelcraftPhase", phase, "-pixelcraftRoute", route, "-logFile", str(log),
                ], cwd=project, timeout=600)
            except subprocess.TimeoutExpired:
                failed[route] = f"Unity {route} {phase} timed out. See {log}"
                continue
            contents = log.read_text(encoding="utf-8", errors="replace") if log.exists() else ""
            for line in contents.splitlines():
                if line.startswith(("PC_SHEET ", "PC_ASE ")):
                    print(line, flush=True)
            marker = f"PC_{'SHEET' if route == 'sheet' else 'ASE'} {phase} passed"
            if result.returncode or not re.search("^" + re.escape(marker) + "$", contents, re.M):
                failed[route] = f"Unity {route} {phase} did not finish successfully. See {log}"
    if failed:
        raise SystemExit("\n".join(failed.values()))


if __name__ == "__main__":
    main()
