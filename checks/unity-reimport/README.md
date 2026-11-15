# PixelCraft Unity reimport check

Run from the PixelCraft repository root with Python 3, a built PixelCraft binary, and an activated Unity Editor. The template targets Unity 6.3 LTS 6000.3.28f1 and pins Unity's official 2D Aseprite Importer to 3.0.2. Unity needs access to its package registry on the first run.

```sh
cargo build --locked
python3 checks/unity-reimport/run.py --unity /path/to/Unity --project /path/to/new-test-project
```

The check creates a temporary fixture within the chosen project. It starts with six unevenly timed frames and three tags. It exports one PNG, JSON and Unity meta file, redraws one frame with wider painted bounds, then inserts a frame into walk and changes the duration of the surviving redrawn frame. Each phase overwrites the same asset paths and opens Unity against the same project. The script keeps the project, package lock, logs, and asset IDs from the first import for inspection.

The sheet check uses the JSON importer to generate the three clips. It compares their sprite order, millisecond key times, and duration. It creates a controller on the first run and reloads it on each later run to check that the clip and sprite GUIDs and local file IDs survive. It samples the controller's walk clip and checks the resolved sprite and its source PNG pixels. The redraw leaves the untrimmed cell geometry fixed. Insertion shifts later cells while retaining their sprite references.

The official Aseprite route imports a separate `Assets/Direct/hero.aseprite` file from the same PixelCraft source. It checks that the official importer produces the sprites and named clips, then reloads its own existing Animator Controller and reports resolved motions, actual clip timing, and subasset identity changes. It also tracks the final walk sprite through insertion because matching the same ordinal name can miss a change in artwork identity.

Use `--route sheet` or `--route aseprite` to run one route in a new project. Use `--fixtures-only` to generate and keep the three stage exports under `Fixture/` without starting Unity. The sheet check invokes the same importer entry point used when selecting the JSON in Assets and choosing **Assets → PixelCraft → Import or Update Animation Clips**.

See the [engine verification results](../../integrations/unity/README.md#verified-in-unity-6000328f1-on-windows-and-linux).
