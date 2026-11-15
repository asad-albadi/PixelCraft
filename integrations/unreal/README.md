# PixelCraft Flipbooks for Unreal Engine

The editor script turns PixelCraft animation tags into Paper2D Flipbook assets.
Tags such as `idle`, `walk`, `jump`, `attack`, and `throw` all use the same
workflow. Simple lowercase tag names become asset names directly. Other names
get a stable Unreal-compatible asset name and retain the exact label in the
`PixelCraftTag` metadata field.

## Install once per Unreal project

1. Enable **Paper2D**, **Python Editor Script Plugin**, and **Editor Scripting
   Utilities** in Unreal.
2. Copy [pixelcraft_flipbooks.py](pixelcraft_flipbooks.py) into
   `<Unreal project>/Content/Python/`. Add these lines to
   `Content/Python/init_unreal.py`, creating the file if needed.

   ```python
   import pixelcraft_flipbooks
   pixelcraft_flipbooks.install()
   ```

3. Restart Unreal Editor.

## Import and reimport

1. In PixelCraft, choose **File → Export sprite sheet**. Choose **Generic**
   and **All frames and tags**. To keep cell sizes and positions on redraws,
   disable trimming and reuse the same columns, padding, and scale.
2. Keep the PNG and JSON together. Give the JSON the `.paper2dsprites`
   extension and import that file into Unreal's Content Browser. Paper2D
   imports the textures and sprites. The script creates the Flipbooks in a
   `<sheet>_Flipbooks` folder beside the sheet asset. Use **Save All**.
3. After editing the same native PixelCraft project, export over the same PNG
   and replace the `.paper2dsprites` file with the new JSON. In the Content
   Browser, right-click the **PaperSpriteSheet** asset and choose **Reimport**.
   The importer refreshes existing Flipbooks at their current asset paths and
   creates assets for new tags. Use **Save All** to save the reimported sheet,
   sprites, and texture.

Keep the PaperSpriteSheet asset at the same Unreal path and keep tag names
stable to preserve Flipbook references. Renaming a tag creates a new asset.
The script keeps the old asset when a tag disappears, so you can migrate any
references before removing it. The importer updates the sprite keys and
timing, while material and collision settings belong to the existing
Flipbook.

Flipbook keys use one timing tick per millisecond and preserve the order,
direction, and positive repeat counts exported by PixelCraft. A tag set to
loop indefinitely imports as one cycle. Set **Looping** on the
**PaperFlipbookComponent** to choose whether that cycle loops in your game.
The integration accepts up to 65,535 frames and 1,024 animation tags per sheet.

For a sheet that was already imported before installing the script, reimport
it or run the following in the Unreal Python console after selecting one
PaperSpriteSheet asset.

```python
import unreal
import pixelcraft_flipbooks

sheet, = unreal.EditorUtilityLibrary.get_selected_assets()
pixelcraft_flipbooks.sync_flipbooks(sheet)
```

The [Unreal editor check](../../checks/unreal-reimport/README.md) tests timing,
custom tags, same-path reimport, and references from an existing level.
