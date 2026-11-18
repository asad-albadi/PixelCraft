#!/usr/bin/env python3
"""External client example. PixelCraft evaluates only typed native requests."""
import json
from pathlib import Path
import random
import subprocess
import sys


def run(executable, operation, arguments):
    result = subprocess.run([executable, "--cli", operation, json.dumps(arguments)], check=True, capture_output=True, text=True)
    return json.loads(result.stdout)


def main():
    if len(sys.argv) != 3:
        raise SystemExit("Usage: native-workflow.py PIXELCRAFT NEW_OUTPUT_DIRECTORY")
    executable = str(Path(sys.argv[1]).resolve())
    directory = Path(sys.argv[2]).resolve()
    directory.mkdir(parents=True, exist_ok=False)
    source, painted = directory / "source.pixel", directory / "painted.pixel"
    run(executable, "create", {"width":32, "height":24, "output":str(source)})
    revision = run(executable, "inspect", {"input":str(source)})["revision"]
    rng = random.Random(23)
    commands = [{"op":"add-layer", "name":"Native marks"}, {"op":"add-frame", "frame":0}]
    for frame in range(2):
        for _ in range(24):
            x, y = rng.randrange(32), rng.randrange(24)
            commands.append({"op":"shape", "layer":1, "frame":frame, "kind":"line", "x":x, "y":y, "x2":x, "y2":y, "color":[220,160,60,255]})
    run(executable,"commands",{"input":str(source),"output":str(painted),"revision":revision,"commands":commands})
    run(executable,"export",{"input":str(painted),"output":str(directory/"animation.gif"),"format":"gif","scale":4})
    print(json.dumps({"project":str(painted),"gif":str(directory/"animation.gif")}))


if __name__ == "__main__":
    main()
