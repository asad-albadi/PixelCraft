# Tablet interaction research and decisions

2026-11-22. User feedback: the initial Sprite tools property form is unintuitive;
color selection needs a visual picker; add a focused Zen drawing mode. This
extends interface milestone10 without changing the14-milestone denominator.

## Evidence

- Procreate shows a visible layer stack, thumbnails, visibility and per-layer
  options: https://help.procreate.com/procreate/handbook/layers/layers-interface
- Pixaki uses layer lists/actions and a scrub-friendly animation panel:
  https://pixaki.com/user-guide/layers/ and https://pixaki.com/user-guide/animation/
- Aseprite combines layer rows with frame columns and contextual cel actions:
  https://aseprite.org/docs/timeline/ and https://aseprite.org/docs/basics/
- Resprite describes layer headers, frame headers, cels and clip ranges, plus
  tool-specific selection modes: https://resprite.fengeon.com/docs/animation/timeline
  and https://resprite.fengeon.com/docs/drawing/tools
- Android recommends adapting components to window space and supporting panes,
  rather than stretching forms: https://developer.android.com/develop/adaptive-apps/guides/adaptive-dos-and-donts
- Android touch target guidance: https://developer.android.com/guide/topics/ui/accessibility/views/apps-views
- Procreate full-screen keeps a visible return control; QuickMenu gives temporary
  access to common actions: https://help.procreate.com/procreate/handbook/actions/actions-preferences
  and https://help.procreate.com/procreate/handbook/interface-gestures/quickmenu
- Krita Canvas Only lets users choose hidden chrome, with Tab/four-finger entry:
  https://docs.krita.org/en/reference_manual/preferences/canvas_only_mode.html
- Krita popup palette provides on-canvas colors, brushes and navigation:
  https://docs.krita.org/en/reference_manual/popup-palette.html
- Sketchbook mobile Rapid UI retains brush controls and an edge trigger for
  Brush/Color/Layers, including handedness placement:
  https://help.sketchbook.com/docs/hiding-the-ui-while-you-draw

Firsthand qualitative feedback (not evidence of how common a problem is):

- Users struggling to escape tablet canvas-only mode:
  https://www.reddit.com/r/krita/comments/sskubi/exiting_canvas_only_mode_on_tablet/
  and https://krita-artists.org/t/android-view-show-canvas-only-how-go-back/93863
- Tablet users missing Undo or finding gesture-only Undo unreliable:
  https://www.reddit.com/r/krita/comments/1iwv5vo/where_the_hell_is_the_undo_button_samsung_tablet/

## PixelCraft decisions (inferences, not claims about those apps)

Keep familiar placement: colors and Preview left, drawing tools right, layers and
frames together at the bottom. The timeline can expand into a layer×frame grid;
its selected rows/cells remain visible rather than buried in a dropdown. Give
layers inline eye/lock controls and an explicit overflow menu. Frame controls
stay on the timeline; use compact contextual menus and small property dialogs.
Show selection operations beside an active selection. Keep visible alternatives
to long-press or drag gestures. Do not require people to discover hidden gestures.

Replace the RGB slider form with saturation/value, hue, checkerboard alpha,
before/after swatches, a name and six-digit RGB; keep opacity separate.

Zen is presentation state over the same editor, never a different document.
Hide normal panels/timeline/status chrome; keep visible Exit Zen, Undo/Redo,
pencil/eraser, color, brush size and temporary access to further tools. Use safe
48dp touch targets, adapt the compact strip to short/narrow windows, allow
handedness placement, consume overlay touches, preserve zoom/pan and editing
targets, and restore panels on exit. Android Back closes transient UI first and
exits Zen before leaving the editor. Keep system navigation discoverable. Start
with explicit toggling; auto-hiding and multi-finger entry can wait.

Validate actual emulator interactions and large text/keyboard layouts. A tested
emulator does not establish stylus ergonomics or physical-device performance.
