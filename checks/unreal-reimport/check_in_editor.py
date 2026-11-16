"""Run inside Unreal Editor. See README.md for the three test phases."""

import json
import math
from pathlib import Path

import unreal
import pixelcraft_flipbooks

PROJECT = Path(unreal.Paths.project_dir()).resolve()
SOURCE = PROJECT / "SourceSheet" / "hero.paper2dsprites"
DEST = "/Game/PixelCraftCheck"
SHEET = f"{DEST}/hero"
FLIPBOOKS = f"{DEST}/hero_Flipbooks"
LEVEL = f"{DEST}/ExistingReferences"
BASELINE = PROJECT / "reference_paths.json"

INITIAL = {
    "idle": [(0, 93), (1, 147)],
    "walk": [(2, 71), (3, 129)],
    "jump": [(4, 121), (5, 203)],
    "attack": [(4, 121), (3, 129)],
    "throw": [(2, 71), (3, 129), (4, 121), (3, 129)] * 2,
}
UPDATED = {
    **INITIAL,
    "walk": [(2, 71), (6, 111), (3, 173)],
    "attack": [(4, 121), (3, 173)],
    "throw": [(2, 71), (6, 111), (3, 173), (4, 121), (3, 173), (6, 111)] * 2,
    "dash": [(5, 203)],
}


def check_source_limits():
    def source(frames, tags):
        return json.dumps({
            "frames": frames, "meta": {"app": "PixelCraft", "version": "1", "frameTags": tags},
        }).encode("utf-8")

    for payload, message in [
        (source([{}], [{}] * 1025), "1024 animation tags"),
        (source([{}] * 65536, []), "1 to 65535 frames"),
        (source([{}], {}), "1024 animation tags"),
    ]:
        try:
            pixelcraft_flipbooks._parse_source(payload)
        except ValueError as error:
            assert message in str(error), error
        else:
            raise AssertionError("Accepted unbounded or malformed sheet metadata")
    assert pixelcraft_flipbooks._parse_source(source([{}], [{}] * 1024)) is not None


def first_import():
    check_source_limits()
    task = unreal.AssetImportTask()
    task.set_editor_properties({
        "filename": str(SOURCE), "destination_path": DEST, "automated": True,
        "factory": unreal.PaperSpriteSheetImportFactory(), "save": True,
    })
    unreal.AssetToolsHelpers.get_asset_tools().import_asset_tasks([task])
    assert isinstance(unreal.load_asset(SHEET), unreal.PaperSpriteSheet)


def check_flipbooks(expected):
    sheet = unreal.load_asset(SHEET)
    sprites = dict(zip(sheet.get_editor_property("sprite_names"), sheet.get_editor_property("sprites")))
    frame_rects = {
        value["filename"]: tuple(value["frame"][key] for key in ("x", "y", "w", "h"))
        for value in json.loads(SOURCE.read_text())["frames"]
    }
    assert set(sprites) == set(frame_rects)
    for name, sprite in sprites.items():
        uv = sprite.get_editor_property("source_uv")
        size = sprite.get_editor_property("source_dimension")
        assert (uv.x, uv.y, size.x, size.y) == frame_rects[name], name

    paths = {}
    for name, keyframes in expected.items():
        clip = unreal.load_asset(f"{FLIPBOOKS}/{name}")
        assert isinstance(clip, unreal.PaperFlipbook), f"Missing Flipbook {name}"
        keys = clip.get_editor_property("key_frames")
        assert clip.get_editor_property("frames_per_second") == 1000
        assert [(key.get_editor_property("sprite").get_name(),
                 key.get_editor_property("frame_run")) for key in keys] == [
            (f"frame_{index}", duration) for index, duration in keyframes
        ], name
        elapsed = 0
        for index, duration in keyframes:
            assert clip.get_sprite_at_time((elapsed + duration / 2) / 1000) == sprites[f"frame_{index}"], name
            elapsed += duration
        assert math.isclose(clip.get_total_duration(), elapsed / 1000, abs_tol=0.00001), name
        paths[name] = clip.get_path_name()
        unreal.log(f"PC_FLIPBOOK {name}: {len(keys)} keys, {elapsed} ms")
    special = [unreal.load_asset(path) for path in unreal.EditorAssetLibrary.list_assets(FLIPBOOKS)]
    assert sum(unreal.EditorAssetLibrary.get_metadata_tag(clip, "PixelCraftTag") == "Heavy Throw!"
               for clip in special) == 1
    return paths, {name: sprite.get_path_name() for name, sprite in sprites.items()}


def baseline():
    paths, sprites = check_flipbooks(INITIAL)
    levels = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
    assert levels.new_level(LEVEL)
    actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
    for index, (name, path) in enumerate(paths.items()):
        actor = actors.spawn_actor_from_class(unreal.PaperFlipbookActor, unreal.Vector(index * 40, 0, 0))
        actor.set_actor_label(name)
        actor.get_component_by_class(unreal.PaperFlipbookComponent).set_flipbook(unreal.load_asset(path))
    assert levels.save_current_level()
    BASELINE.write_text(json.dumps({"flipbooks": paths, "sprites": sprites}))
    unreal.log("PC_FLIPBOOK_BASELINE passed")


def reimported():
    paths, sprites = check_flipbooks(UPDATED)
    baseline = json.loads(BASELINE.read_text())
    for name, path in baseline["flipbooks"].items():
        assert paths[name] == path, name
    for name, path in baseline["sprites"].items():
        assert sprites[name] == path, name
    levels = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
    assert levels.load_level(LEVEL)
    actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem).get_all_level_actors()
    references = {}
    for actor in actors:
        if isinstance(actor, unreal.PaperFlipbookActor):
            flipbook = actor.get_component_by_class(unreal.PaperFlipbookComponent).get_flipbook()
            references[actor.get_actor_label()] = flipbook.get_path_name()
    assert references == baseline["flipbooks"]
    assert unreal.load_asset(references["walk"]).get_num_frames() == 355
    unreal.log("PC_FLIPBOOK_REIMPORT passed: 5 existing level references and 6 original sprite paths")
