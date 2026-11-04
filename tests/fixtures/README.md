# Compatibility fixtures

The .aseprite projects are unmodified public upstream test projects, retrieved
from https://github.com/aseprite/aseprite/tree/main/tests/sprites on 2026-11-02.
They retain upstream ownership (Igara Studio / contributors) and are test inputs,
not PixelCraft artwork or runtime resources. See upstream.json for source links,
Git blob IDs, file SHA-256 and reference-sheet SHA-256.

rendered/ contains reference RGBA PNG sheets generated once with Aseprite
1.3.18.6-dev. RGB conversion before export is intentional: exporting an indexed
sheet can mark palette index zero transparent even on a background layer.
The normal test suite reads the PNGs directly and never invokes Aseprite.

scripts/ contains unchanged examples from Aseprite-Script-Examples (MIT).
The original license is scripts/LICENSE.txt. Tests supply user responses via the
same host callback used by the editor, and run the example code unchanged.

The fixture files are not embedded in or read by the shipped executable.

workflows/ contains original tiny fixtures generated for the expanded import tests:
a 3×2 APNG, a lossless animated WebP (red then green, 70/230 ms), and a two-layer
OpenRaster ZIP (Red and half-opacity Green). They were generated with Pillow and
Python zipfile, contain no third-party artwork, and are only test inputs.
