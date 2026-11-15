"""Unreal Editor integration for PixelCraft's Paper2D sprite sheets."""

import hashlib
import json
from pathlib import Path
import re

import unreal

SHEET_TAG = "PixelCraftSheet"
NAME_TAG = "PixelCraftTag"
MAX_FRAMES = 65_535
MAX_TAGS = 1024
MAX_KEYS = 1_000_000
MAX_DURATION = 2_147_483_647
SIMPLE_NAME = re.compile(r"[a-z0-9]+(?:_[a-z0-9]+)*")
DIRECTIONS = {"forward", "reverse", "pingpong", "pingpong_reverse"}

_imports = None
_pending = {}
_tick = None


def _asset_name(tag):
    # Unreal names are case insensitive. Keep simple lowercase names readable
    # and reserve a separate namespace for names that need encoding.
    if len(tag) <= 48 and SIMPLE_NAME.fullmatch(tag):
        return tag
    stem = re.sub(r"[^a-z0-9]+", "_", tag.lower()).strip("_")[:48] or "animation"
    digest = hashlib.sha256(tag.encode("utf-8")).hexdigest()[:16]
    return f"{stem}__{digest}"


def _read_source(sheet):
    source = Path(sheet.get_editor_property("asset_import_data").get_first_filename())
    if source.suffix.lower() not in {".paper2dsprites", ".json"} or not source.is_file():
        return None
    with source.open("rb") as file:
        payload = file.read(64 * 1024 * 1024 + 1)
    if len(payload) > 64 * 1024 * 1024:
        raise ValueError("Paper2D source exceeds 64 MiB")
    return _parse_source(payload)


def _parse_source(payload):
    data = json.loads(payload)
    if not isinstance(data, dict):
        return None
    meta = data.get("meta", {})
    if not isinstance(meta, dict) or meta.get("app") != "PixelCraft":
        return None
    if meta.get("version") != "1":
        raise ValueError("Unsupported PixelCraft sprite sheet version")
    frames = data.get("frames")
    tags = meta.get("frameTags", [])
    if not isinstance(frames, list) or not 1 <= len(frames) <= MAX_FRAMES:
        raise ValueError(f"A PixelCraft sheet must contain 1 to {MAX_FRAMES} frames")
    if not isinstance(tags, list) or len(tags) > MAX_TAGS:
        raise ValueError(f"A PixelCraft sheet supports up to {MAX_TAGS} animation tags")
    return frames, tags


def _sprite_frames(sheet, frames):
    filenames = [frame["filename"] for frame in frames]
    imported_names = list(sheet.get_editor_property("sprite_names"))
    imported_sprites = list(sheet.get_editor_property("sprites"))
    sprites = dict(zip(imported_names, imported_sprites))
    if len(sprites) != len(filenames) or len(set(filenames)) != len(filenames):
        raise ValueError("Sprite names in the sheet and source do not match")
    result = []
    for frame in frames:
        name = frame["filename"]
        sprite = sprites.get(name)
        if not isinstance(sprite, unreal.PaperSprite):
            raise ValueError(f"Missing imported PaperSprite {name!r}; reimport the sheet first")
        uv = sprite.get_editor_property("source_uv")
        size = sprite.get_editor_property("source_dimension")
        if (uv.x, uv.y, size.x, size.y) != tuple(frame["frame"][key] for key in ("x", "y", "w", "h")):
            raise ValueError(f"Sprite bounds have changed for {name!r}; reimport the sheet first")
        duration = frame["duration"]
        if type(duration) is not int or not 1 <= duration <= 65535:
            raise ValueError(f"Invalid duration for {name!r}")
        result.append((sprite, duration))
    return result


def _plan(sheet, frames, tags):
    package, name = sheet.get_path_name().rsplit(".", 1)
    folder = f"{package.rsplit('/', 1)[0]}/{name}_Flipbooks"
    library = unreal.EditorAssetLibrary
    ownership = sheet.get_path_name()
    taken = set()
    budget = MAX_KEYS
    plan = []
    for tag in tags:
        label = tag["name"]
        if not isinstance(label, str) or not label.strip():
            raise ValueError("Each animation tag needs a nonempty name")
        asset_name = _asset_name(label)
        if asset_name in taken:
            raise ValueError(f"Duplicate animation tag or asset name for {label!r}")
        taken.add(asset_name)

        start, end = tag["from"], tag["to"]
        direction, repeat = tag["direction"], tag.get("repeat", 0)
        if (type(start) is not int or type(end) is not int or not 0 <= start <= end < len(frames)
                or direction not in DIRECTIONS or type(repeat) is not int or not 0 <= repeat <= 65535):
            raise ValueError(f"Invalid animation range, direction or repeat count for {label!r}")
        cycle = frames[start:end + 1]
        if direction in {"reverse", "pingpong_reverse"}:
            cycle.reverse()
        if direction in {"pingpong", "pingpong_reverse"}:
            cycle.extend(cycle[-2:0:-1])
        cycles = max(repeat, 1)
        key_count = len(cycle) * cycles
        if key_count > budget or sum(duration for _, duration in cycle) * cycles > MAX_DURATION:
            raise ValueError(f"Animation exceeds the Flipbook timing or key limit: {label!r}")
        budget -= key_count

        path = f"{folder}/{asset_name}"
        existing = library.load_asset(path) if library.does_asset_exist(path) else None
        if existing is not None and (
            not isinstance(existing, unreal.PaperFlipbook)
            or library.get_metadata_tag(existing, SHEET_TAG) != ownership
            or library.get_metadata_tag(existing, NAME_TAG) != label
        ):
            raise ValueError(f"An unrelated asset occupies {path}")
        plan.append((asset_name, label, existing, cycle, cycles))
    return folder, plan


def sync_flipbooks(sheet):
    """Create or refresh the tagged Flipbooks for an imported PaperSpriteSheet."""
    if not isinstance(sheet, unreal.PaperSpriteSheet):
        raise TypeError("Select a PaperSpriteSheet asset")
    source = _read_source(sheet)
    if source is None:
        return []
    frames, tags = source
    folder, plan = _plan(sheet, _sprite_frames(sheet, frames), tags)
    assets = []
    tools = unreal.AssetToolsHelpers.get_asset_tools()
    library = unreal.EditorAssetLibrary
    for name, label, existing, cycle, count in plan:
        clip = existing
        if clip is None:
            clip = tools.create_asset(name, folder, unreal.PaperFlipbook, unreal.PaperFlipbookFactory())
            if clip is None:
                raise RuntimeError(f"Cannot create {folder}/{name}")
            library.set_metadata_tag(clip, SHEET_TAG, sheet.get_path_name())
            library.set_metadata_tag(clip, NAME_TAG, label)
        # The Flipbook's integer frame runs represent milliseconds at 1000 FPS.
        keys = []
        for sprite, duration in cycle:
            key = unreal.PaperFlipbookKeyFrame()
            key.set_editor_property("sprite", sprite)
            key.set_editor_property("frame_run", duration)
            keys.append(key)
        clip.modify()
        clip.set_editor_properties({"frames_per_second": 1000.0, "key_frames": keys * count})
        assets.append(clip)
    if assets:
        unreal.log(f"PixelCraft: synced {len(assets)} Flipbooks in {folder}")
    return assets


def _sync_pending(_delta_seconds):
    global _tick
    unreal.unregister_slate_post_tick_callback(_tick)
    _tick = None
    sheets = list(_pending.values())
    _pending.clear()
    for sheet in sheets:
        try:
            sync_flipbooks(sheet)
        except Exception as error:
            unreal.log_error(f"PixelCraft: {sheet.get_path_name()}: {error}")


def _queue(asset):
    global _tick
    if isinstance(asset, unreal.PaperSpriteSheet):
        _pending[asset.get_path_name()] = asset
        if _tick is None:
            # Let Paper2D finish updating its frames and the registry first.
            _tick = unreal.register_slate_post_tick_callback(_sync_pending)


def _post_import(_factory, asset):
    _queue(asset)


def install():
    """Listen to native Paper2D imports and reimports for this editor session."""
    global _imports
    if _imports is None:
        _imports = unreal.get_editor_subsystem(unreal.ImportSubsystem)
        _imports.on_asset_post_import.add_callable_unique(_post_import)
        _imports.on_asset_reimport.add_callable_unique(_queue)
