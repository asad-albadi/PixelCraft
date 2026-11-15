# Updating PixelCraft animations in Unity

The importer targets Unity 6. The test project pins Unity **6000.3.28f1** and Unity's **2D Aseprite Importer 3.0.2**. The Aseprite package is only needed for the separate direct ASE test.

1. Copy [PixelCraftSpriteSheetImporter.cs](Editor/PixelCraftSpriteSheetImporter.cs) into an `Assets/Editor` folder in your Unity project.
2. In PixelCraft, save the native `.pixel` project. Use **File → Export sprite sheet**, select **Unity** and **All frames and tags**, and leave **Trim transparent borders** off. Choose the columns, padding, and scale you want to keep on re-export.
3. Export the PNG into a folder under Unity's `Assets`. Keep the PNG, JSON, and `.png.meta` together. In Unity's Project window, select the JSON and choose **Assets → PixelCraft → Import or Update Animation Clips**.
4. Add the clips from the adjacent `<sheet>.PixelCraftClips` folder to your Animator Controller. The clips animate a `SpriteRenderer` on the same GameObject as the Animator.
5. After an edit, replace the same sheet files using the same native project and export settings. Run the Unity command on the same JSON again.

Give every exported tag a unique, non-empty name. The importer updates the existing generated `.anim` assets by tag name. It replaces each generated sprite curve and updates its loop setting. Keep the sheet, generated clip folder, and tag names at their original paths and values. Renamed tags get new clips. Removed tags leave their previous clips on disk so existing references survive. An unrelated asset at a generated clip path causes the import to stop before modifying clips.

A redraw keeps fixed-cell rectangles in place. Inserting or deleting frames can move rectangles, while surviving frames keep their sprite identities. A deleted frame has no corresponding sprite in the refreshed sheet. PixelCraft's timeline currently offers frame insertion and deletion. Treat earlier single-tag PixelCraft Unity sheets as a migration case, since their original Unity names and IDs came from positions in the selected range.

Tags run forward, in reverse, or in either ping-pong direction. A repeat count of zero loops. A positive count plays that many cycles and then stops. Sprite keys use milliseconds, and the last sprite is held for its authored duration. The project and clip remain separate assets so the controller can refer to the same clip on each import.

**View → Seamless preview** lets you check both edges in a 3×3 grid. A normal PNG export writes the source frame once. [Preview validation](../../checks/seamless-preview-validation.md)

## Reproduce both import routes

Build PixelCraft, then run the following from the repository in a terminal with Python 3:

```sh
cargo build --locked
python3 checks/unity-reimport/run.py \
  --unity /path/to/Unity \
  --importer integrations/unity/Editor/PixelCraftSpriteSheetImporter.cs \
  --project /path/to/new-unity-reimport-check
```

On Windows, pass the full paths to `Unity.exe` and `pixelcraft.exe` with `--unity` and `--pixelcraft`. Use an activated Editor. The runner generates the same three-tag project with unequal durations, redraws a frame, and then inserts and retimes a frame. It uses the same Unity project and asset paths throughout. The sheet route checks the generated clips, sprite and clip asset IDs, and the existing Animator Controller. The direct ASE route uses Unity's official importer and reports its clips and asset references independently. Use `--route sheet` or `--route aseprite` to isolate a route and `--fixtures-only` to generate the inputs without Unity.

The direct importer is a separate workflow. Version 3.0.2 reads tags and frame durations. Its clip generator treats zero repeats as looping and positive counts as non-looping. Use the sheet importer when authored direction, a precise finite repeat count, or stable references to individual sprites across insertions matter.

### Verified in Unity 6000.3.28f1 on Linux

Both import routes passed initial import, same-path redraw, and frame insertion with retiming. Each existing Animator Controller retained all three clip references.

- **PNG and JSON.** The redraw changed the artwork and preserved its fixed atlas cells. After insertion, all six surviving sprites kept their Unity GUIDs and local IDs, and all three animation clips kept theirs. The controller's walk clip sampled the redrawn sprite at its new position in the sequence.
- **Official Aseprite importer 3.0.2.** The existing controller retained its three clips. The walk clip updated from 200 ms to 355 ms. The original last walk sprite moved from `Frame_3` to `Frame_4` and received a different local ID. A direct reference to that sprite does not track the underlying artwork across insertion.
