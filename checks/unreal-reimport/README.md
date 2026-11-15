# Verify PixelCraft Flipbook reimport in Unreal

Use a disposable Unreal project with Paper2D, Python Editor Script Plugin, and
Editor Scripting Utilities enabled. Install the
[PixelCraft editor script](../../integrations/unreal/README.md), then build
the PixelCraft debug binary and run this from the PixelCraft checkout.

```sh
python checks/unreal-reimport/fixtures.py /path/to/disposable-unreal-project
```

This exports the real PixelCraft PNG and metadata for two versions of the
same project. The second redraws one frame, inserts another, changes walk
timing, and adds a `dash` tag.

In Unreal Editor's Python console, load the verification script.

```python
check = {}
exec(open(r"/path/to/pixelcraft/checks/unreal-reimport/check_in_editor.py").read(), check)
check["first_import"]()
```

After the Output Log reports six synced Flipbooks, run `check["baseline"]()`.
The check measures each Flipbook and saves a level with five actors that already
reference the imported assets. Save All in Unreal.

Copy both files from the temporary project's `UpdatedSheet` directory over the
ones in `SourceSheet`. In Unreal's Content Browser, right-click
`/Game/PixelCraftCheck/hero` and choose **Reimport**. After the Output Log
reports seven synced Flipbooks, save all and run `check["reimported"]()`.
The check verifies frame names and rectangles, frame order and duration for
each named Flipbook, the original sprite asset paths, and the references in the
saved level. The expected Output Log result is `PC_FLIPBOOK_REIMPORT passed`.

Verified in Unreal Engine 5.2.0 on Linux. Native Reimport kept all five
saved level references and all six original PaperSprite paths. The walk
Flipbook changed from 200 ms to 355 ms, the reverse attack from 250 ms to
294 ms, and the repeated ping-pong throw from 900 ms to 1520 ms. The new
`dash` tag created a 203 ms Flipbook.
