# Native window decorations validation

Windows and Linux now request native window decorations from eframe/winit. The custom title bar, window resize hit regions, drag/maximize/minimize/close buttons and title-bar double-click handler were deleted. The existing native close-request route still cancels a close and prompts for every dirty tab. The inspector header owns its collapse button; a 32-point left strip keeps a restore button visible while hidden. View menu commands share the same inspector state.

Verification:
- Header/restore pointer tests at physical 1366×768 and 1920×1080 with native scales 100%, 125%, 150%, 175%, 200%; inspector and timeline drag resizing; bottom-left preview docking.
- Native close events with active and background dirty tabs, Cancel preserves edits, Discard checks all remaining dirty tabs before sending Close; actual View menu hide/show clicks.
- Full offline locked Cargo suite, formatting, strict all-target Clippy and build logs: native-decorations-{tests,clippy,build}.log.
- Fresh software renders of the actual egui meshes/font atlas: shots/native-decorations, including open display-scale matrix and collapsed 100%/200% views. Reviewed client render and both collapsed views: no custom title bar, visible restore button, right tools and lower timeline retained.
- Craft Anything rebuild_check passed build and app startup screenshot (1702×1365 client image), at ../checks/round-0/screenshot.png. Its original mechanics runtime comparisons and broad file/extension parity remain unverified.

Limits: This sandbox denies connection to the Linux desktop socket; no Windows host/toolchain is installed. Native OS frame appearance, dragging, border resizing, minimize/maximize, double-click, native close-button and Alt+F4 interactions therefore still need interactive verification on both Windows and Linux, at the requested sizes/scales. Client-area screenshots omit the OS-owned decoration and cannot establish native frame rendering. The automated close-request tests exercise the application side of the same native event route, rather than claiming native system interaction was tested.

The first unrestricted-data test attempt hit a read-only preferences directory in native_assets_apply_inside_workshop_and_shortcuts_remain_local; reruns use temporary PIXELCRAFT_HOME and XDG_CONFIG_HOME directories. No user preferences need to be changed.
