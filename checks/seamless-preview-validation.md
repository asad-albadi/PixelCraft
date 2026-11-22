# Seamless Preview and PNG export

Open **View → Seamless preview** to see the active frame repeated in three rows and three columns. The preview keeps the source aspect ratio and fits the entire grid into the available screen height. **File → Export PNG…** writes the active frame at the selected export scale.

Run `cargo test --locked -p pixelcraft-desktop --lib seamless_preview` to reproduce the checks. The rendering test uses a 3 × 2 active frame with six distinct colors so all opposite edges differ. It checks nine complete, adjacent copies with the same orientation and aspect ratio. It then keeps the preview open, exports a PNG at 1× through the editor's export worker, and checks the 3 × 2 dimensions and each source pixel. It then matches both pairs of opposite edges and checks that all nine copies use the corrected tile. A second test verifies that all nine copies of a 4 × 40 sprite remain visible in an 800 × 480 viewport. Both checks use egui's rendered output on Linux.
