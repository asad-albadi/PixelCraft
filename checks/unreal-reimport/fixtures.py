"""Create both real PixelCraft exports in an existing disposable Unreal project."""

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("project", type=Path)
parser.add_argument("--pixelcraft", type=Path, default=Path("target/debug/pixelcraft"))
args = parser.parse_args()
project = args.project.resolve()
pixelcraft = args.pixelcraft.resolve()

unity_fixture = Path(__file__).resolve().parents[1] / "unity-reimport" / "run.py"
spec = importlib.util.spec_from_file_location("unity_fixture", unity_fixture)
fixtures = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixtures)

# Reuse the common six-frame project and the redraw + insertion from the Unity check.
scratch = project / "FixtureBuild"
(scratch / "Assets").mkdir(parents=True)
for _ in fixtures.fixture_operations(scratch, str(pixelcraft)):
    pass

def cli(operation, **values):
    return json.loads(subprocess.check_output(
        [pixelcraft, "--cli", operation, json.dumps(values)], text=True,
    ))

for phase, (source, attack, throw) in {
    "SourceSheet": ("initial", (3, 4), (2, 4)),
    "UpdatedSheet": ("insert", (4, 5), (2, 5)),
}.items():
    source_path = scratch / "Fixture" / f"{source}.pixel"
    output_project = scratch / f"{phase}.pixel"
    output = project / phase
    output.mkdir(exist_ok=True)
    tags = [("attack", *attack, 1, 1), ("throw", *throw, 2, 2), ("Heavy Throw!", 0, 0, 3, 0)]
    if source == "insert":
        tags.append(("dash", 6, 6, 0, 0))
    cli("commands", input=str(source_path), output=str(output_project),
        revision=cli("inspect", input=str(source_path))["revision"],
        commands=[
            {"op": "add-tag", "tag": dict(name=name, direction=direction, repeat=repeat,
                                         color=[100, 190, 170], **{"from": start, "to": end})}
            for name, start, end, direction, repeat in tags
        ])
    cli("export", input=str(output_project), output=str(output / "hero.png"), format="sheet",
        options={"preset": "Generic", "trim": False, "padding": 0, "scale": 1, "columns": 4})
    (output / "hero.json").rename(output / "hero.paper2dsprites")
