import 'dart:async';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';

import '../editor_controller.dart';
import 'document_tools.dart';

class PixelCanvas extends StatefulWidget {
  final EditorController editor;
  const PixelCanvas({super.key, required this.editor});
  @override
  State<PixelCanvas> createState() => _PixelCanvasState();
}

class _PixelCanvasState extends State<PixelCanvas> {
  late final transform = widget.editor.canvasTransform;
  final Set<int> pointers = {};
  int? stylusPointer;
  bool mousePan = false;
  int get generation => editor.canvasDocumentGeneration;
  set generation(int value) => editor.canvasDocumentGeneration = value;
  Size get viewport => editor.canvasViewport;
  set viewport(Size value) => editor.canvasViewport = value;
  bool get grid => editor.pixelGrid;
  static const cell = 8.0;
  EditorController get editor => widget.editor;
  @override
  void dispose() {
    super.dispose();
  }

  void fit(Size size) {
    editor.canvasViewport = size;
    editor.fitCanvas();
  }

  double? pressure(PointerEvent event) {
    if (event.kind != ui.PointerDeviceKind.stylus &&
        event.kind != ui.PointerDeviceKind.invertedStylus) {
      return null;
    }
    final range = event.pressureMax - event.pressureMin;
    if (!range.isFinite || range <= 0 || !event.pressure.isFinite) return null;
    return ((event.pressure - event.pressureMin) / range).clamp(0, 1);
  }

  Offset? pixel(Offset local) {
    final scene = transform.toScene(local);
    final p = Offset(
      (scene.dx / cell).floorToDouble(),
      (scene.dy / cell).floorToDouble(),
    );
    return p.dx >= 0 && p.dy >= 0 && p.dx < editor.width && p.dy < editor.height
        ? p
        : null;
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: editor.canvasUpdates,
    builder: (context, _) => LayoutBuilder(
      builder: (context, constraints) {
        final size = constraints.biggest;
        final imageSize = Size(
          editor.width.toDouble(),
          editor.height.toDouble(),
        );
        if (generation != editor.documentGeneration ||
            viewport != size ||
            editor.canvasDocumentSize != imageSize) {
          final reset =
              generation != editor.documentGeneration ||
              viewport.isEmpty ||
              editor.canvasDocumentSize != imageSize;
          final previous = viewport;
          generation = editor.documentGeneration;
          viewport = size;
          editor.canvasDocumentSize = imageSize;
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (!mounted ||
                viewport != size ||
                editor.canvasDocumentSize != imageSize) {
              return;
            }
            if (reset) {
              fit(size);
            } else {
              transform.value =
                  Matrix4.translationValues(
                    (size.width - previous.width) / 2,
                    (size.height - previous.height) / 2,
                    0,
                  ) *
                  transform.value;
            }
          });
        }
        return ColoredBox(
          color: Theme.of(context).brightness == Brightness.dark
              ? const Color(0xff171b21)
              : const Color(0xffe3e7ed),
          child: Stack(
            children: [
              Positioned.fill(
                child: Listener(
                  key: const Key('pixel-canvas'),
                  onPointerDown: (e) {
                    final erasing =
                        e.kind == ui.PointerDeviceKind.invertedStylus ||
                        (e.kind == ui.PointerDeviceKind.stylus &&
                            (e.buttons & kPrimaryStylusButton) != 0);
                    final pen =
                        e.kind == ui.PointerDeviceKind.stylus ||
                        e.kind == ui.PointerDeviceKind.invertedStylus;
                    if (stylusPointer != null &&
                        e.kind == ui.PointerDeviceKind.touch) {
                      return;
                    }
                    if (e.kind == ui.PointerDeviceKind.mouse &&
                        (e.buttons & kPrimaryMouseButton) == 0 &&
                        (e.buttons & kMiddleMouseButton) == 0) {
                      return;
                    }
                    if (pen) stylusPointer = e.pointer;
                    mousePan =
                        e.kind == ui.PointerDeviceKind.mouse &&
                        (e.buttons & kMiddleMouseButton) != 0;
                    pointers.add(e.pointer);
                    setState(() {});
                    if (pointers.length > 1) {
                      unawaited(editor.cancel());
                      setState(() {});
                    } else if (!mousePan && (editor.tool != 'Pan' || erasing)) {
                      final p = pixel(e.localPosition);
                      if (p != null) {
                        editor.begin(p, eraser: erasing, pressure: pressure(e));
                      }
                    }
                  },
                  onPointerMove: (e) {
                    if (pointers.contains(e.pointer) &&
                        pointers.length == 1 &&
                        !mousePan &&
                        editor.drawing) {
                      final p = pixel(e.localPosition);
                      if (p != null) editor.move(p, pressure: pressure(e));
                    }
                  },
                  onPointerUp: (e) {
                    if (!pointers.remove(e.pointer)) return;
                    if (stylusPointer == e.pointer) stylusPointer = null;
                    mousePan = false;
                    if (editor.drawing) unawaited(editor.end());
                    setState(() {});
                  },
                  onPointerCancel: (e) {
                    if (!pointers.remove(e.pointer)) return;
                    if (stylusPointer == e.pointer) stylusPointer = null;
                    mousePan = false;
                    unawaited(editor.cancel());
                    setState(() {});
                  },
                  child: InteractiveViewer(
                    transformationController: transform,
                    constrained: false,
                    panEnabled:
                        !(editor.drawing && stylusPointer != null) &&
                        (mousePan ||
                            editor.tool == 'Pan' ||
                            pointers.length > 1),
                    scaleEnabled: stylusPointer == null,
                    minScale: .1,
                    maxScale: 40,
                    boundaryMargin: const EdgeInsets.all(2000),
                    child: SizedBox(
                      width: editor.width * cell,
                      height: editor.height * cell,
                      child: CustomPaint(
                        painter: _CanvasPainter(
                          editor.onionImage ?? editor.image,
                          grid,
                          Theme.of(context).brightness == Brightness.dark,
                          editor.gestureStart,
                          editor.gestureEnd,
                          editor.drawing ? editor.gestureTool : editor.tool,
                          Color(editor.color),
                          editor.selection,
                          [
                            for (
                              var i = 0;
                              i < editor.referenceImages.length;
                              i++
                            )
                              (
                                editor.referenceImages[i],
                                Map<String, dynamic>.from(
                                  (editor.state['references'] as List)[i],
                                ),
                              ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              if (editor.selection != null)
                Positioned(
                  top: editor.zen ? 64 : 36,
                  left: 8,
                  right: 8,
                  child: Align(
                    alignment: Alignment.topCenter,
                    child: Material(
                      elevation: 3,
                      borderRadius: BorderRadius.circular(12),
                      color: Theme.of(context).colorScheme.surfaceContainer,
                      child: Wrap(
                        children: [
                          for (final item in [
                            ('Copy selection', 'copy', Icons.copy_outlined),
                            ('Cut selection', 'cut', Icons.content_cut),
                            ('Paste', 'paste', Icons.content_paste),
                            ('Flip selection', 'flip-x', Icons.flip),
                            ('Rotate selection', 'rotate', Icons.rotate_right),
                            ('Deselect', 'deselect', Icons.deselect),
                          ])
                            IconButton(
                              tooltip: item.$1,
                              onPressed:
                                  editor.busy ||
                                      editor.drawing ||
                                      (item.$2 == 'paste' &&
                                          editor.state['has_clipboard'] != true)
                                  ? null
                                  : () => editor.selectionAction(item.$2),
                              icon: Icon(item.$3),
                            ),
                          IconButton(
                            tooltip: 'Selection options',
                            onPressed: editor.busy || editor.drawing
                                ? null
                                : () => showDocumentTools(
                                    context,
                                    editor,
                                    tab: 3,
                                  ),
                            icon: const Icon(Icons.more_horiz),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              if (!editor.zen)
                Positioned(
                  top: 12,
                  left: 14,
                  child: Text(
                    '${editor.width} × ${editor.height} px  /  ${editor.state['mode'] ?? 'RGBA'}',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ),
              if (!editor.zen)
                Positioned(
                  right: 12,
                  bottom: 12,
                  child: Material(
                    color: Theme.of(context).colorScheme.surfaceContainer,
                    borderRadius: BorderRadius.circular(8),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        IconButton(
                          tooltip: 'Pixel grid',
                          isSelected: grid,
                          onPressed: editor.togglePixelGrid,
                          icon: const Icon(Icons.grid_4x4, size: 20),
                        ),
                        TextButton.icon(
                          onPressed: () => fit(size),
                          icon: const Icon(Icons.fit_screen, size: 20),
                          label: const Text('Fit'),
                        ),
                      ],
                    ),
                  ),
                ),
              if (!editor.ready)
                const Center(child: CircularProgressIndicator()),
            ],
          ),
        );
      },
    ),
  );
}

class _CanvasPainter extends CustomPainter {
  final ui.Image? image;
  final bool grid, dark;
  final Offset? from, to;
  final String tool;
  final Color preview;
  final Rect? selection;
  final List<(ui.Image, Map<String, dynamic>)> references;
  _CanvasPainter(
    this.image,
    this.grid,
    this.dark, [
    this.from,
    this.to,
    this.tool = '',
    this.preview = Colors.white,
    this.selection,
    this.references = const [],
  ]);
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint();
    for (var y = 0.0; y < size.height; y += 32) {
      for (var x = 0.0; x < size.width; x += 32) {
        paint.color = ((x / 32 + y / 32).toInt().isEven)
            ? (dark ? const Color(0xff343943) : const Color(0xfff2f3f7))
            : (dark ? const Color(0xff444b56) : const Color(0xffdce0e7));
        canvas.drawRect(
          Rect.fromLTWH(
            x,
            y,
            math.min(32, size.width - x),
            math.min(32, size.height - y),
          ),
          paint,
        );
      }
    }
    void drawReferences(bool behind) {
      for (final item in references) {
        final r = item.$2, im = item.$1;
        if (r['visible'] != true || (r['behind'] == true) != behind) continue;
        final scale = (r['scale'] as num).toDouble(),
            sy = (r['scale_y'] as num?)?.toDouble() ?? scale;
        canvas.drawImageRect(
          im,
          Rect.fromLTWH(0, 0, im.width.toDouble(), im.height.toDouble()),
          Rect.fromLTWH(
            (r['x'] as num) * 8,
            (r['y'] as num) * 8,
            im.width * 8 * scale,
            im.height * 8 * sy,
          ),
          Paint()
            ..color = Color.fromARGB(r['opacity'] as int, 255, 255, 255)
            ..filterQuality = FilterQuality.none,
        );
      }
    }

    drawReferences(true);
    if (image != null) {
      canvas.drawImageRect(
        image!,
        Rect.fromLTWH(0, 0, image!.width.toDouble(), image!.height.toDouble()),
        Offset.zero & size,
        Paint()..filterQuality = FilterQuality.none,
      );
    }
    drawReferences(false);
    if (from != null &&
        to != null &&
        ['Line', 'Rectangle', 'Ellipse', 'Select'].contains(tool)) {
      final a = (from! + const Offset(.5, .5)) * 8,
          b = (to! + const Offset(.5, .5)) * 8;
      final p = Paint()
        ..color = preview
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2;
      if (tool == 'Line') {
        canvas.drawLine(a, b, p);
      } else if (tool == 'Ellipse') {
        canvas.drawOval(Rect.fromPoints(a, b), p);
      } else {
        canvas.drawRect(Rect.fromPoints(a, b), p);
      }
    }
    if (selection != null) {
      final box = Rect.fromLTRB(
        selection!.left * 8,
        selection!.top * 8,
        selection!.right * 8,
        selection!.bottom * 8,
      );
      canvas.drawRect(
        box,
        Paint()
          ..color = Colors.white
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.5,
      );
      canvas.drawRect(
        box.inflate(1),
        Paint()
          ..color = Colors.black54
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1,
      );
    }
    if (grid) {
      final lines = Paint()
        ..color = Colors.black26
        ..strokeWidth = .35;
      for (var x = 0.0; x <= size.width; x += 8) {
        canvas.drawLine(Offset(x, 0), Offset(x, size.height), lines);
      }
      for (var y = 0.0; y <= size.height; y += 8) {
        canvas.drawLine(Offset(0, y), Offset(size.width, y), lines);
      }
    }
    canvas.drawRect(
      Offset.zero & size,
      Paint()
        ..style = PaintingStyle.stroke
        ..color = Colors.white30
        ..strokeWidth = 1,
    );
  }

  @override
  bool shouldRepaint(_CanvasPainter old) =>
      old.image != image ||
      old.references != references ||
      old.grid != grid ||
      old.dark != dark ||
      old.from != from ||
      old.to != to ||
      old.tool != tool ||
      old.preview != preview ||
      old.selection != selection;
}
