# Third-party Lua fixtures

These unchanged fixtures run only in the compatibility test suite. The tests
provide their own document and, where needed, return a dialogue response through
PixelCraft's editor host. No Aseprite install or network connection is needed.

| Fixture | Upstream and license | Tested result |
| --- | --- | --- |
| `gasparoken/duplicateLastFrame.lua` | [Gaspar Capello, MIT](https://github.com/Gasparoken/asepriteScripts/blob/2dcb73d45be1b8d5e32cc5550e2d60c2124954c4/duplicateLastFrame.lua) | Copies the final frame's artwork across two layers, including a positioned cel. |
| `christopherwk210/middle-color.lua` | [Chris Anselmo's script collection, MIT](https://github.com/christopherwk210/aseprite-scripts/blob/a0bd40c9aa5f1604a314c1bc1ec89e1d73ef943c/image/middle-color.lua) | Averages the foreground and background colors and reports the new foreground to the editor. |
| `pixeltica/TileIndexOverlay_v1.aseprite-extension` | [Pixeltica, MIT, release 1.0.0](https://github.com/Pixeltica/AsepriteExtensions/releases/tag/1.0.0), [commit 5ece3a5](https://github.com/Pixeltica/AsepriteExtensions/tree/5ece3a5f87d460b3304c87f1c21c4dd7abaad2fb/TileIndexOverlay) | Installs, opens its options dialog, and draws the numbered overlay and contrast layers using a detached RGB scratch sprite. |

Each directory contains its upstream license. The extension archive is the exact
published asset, including its original manifest and shortcut contribution.

```text
1ecca24f345cdfa0e166c4839647e14c13dde5079d13e0d0cb6a19bf9d2b8cf3  gasparoken/duplicateLastFrame.lua
2d6b74c77fa899601a97c2a395b7e6174d7062a3b68eaf35edab9c68e201e638  christopherwk210/middle-color.lua
d92b7dd200edce71e4b349432380aa5a0c2a66b5e155e046fee2ac3b2d157b64  pixeltica/TileIndexOverlay_v1.aseprite-extension
```
