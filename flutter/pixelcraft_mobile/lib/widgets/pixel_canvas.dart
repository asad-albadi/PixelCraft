import 'dart:async';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import '../editor_controller.dart';

class PixelCanvas extends StatefulWidget {
  final EditorController editor;
  const PixelCanvas({super.key, required this.editor});
  @override
  State<PixelCanvas> createState() => _PixelCanvasState();
}

class _PixelCanvasState extends State<PixelCanvas> {
  late final transform = widget.editor.canvasTransform;
  final Set<int> pointers = {};
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
                    pointers.add(e.pointer);
                    if (pointers.length > 1) {
                      unawaited(editor.cancel());
                      setState(() {});
                    } else if (editor.tool != 'Pan') {
                      final p = pixel(e.localPosition);
                      if (p != null) editor.begin(p);
                    }
                  },
                  onPointerMove: (e) {
                    if (pointers.length == 1 && editor.tool != 'Pan') {
                      final p = pixel(e.localPosition);
                      if (p != null) editor.move(p);
                    }
                  },
                  onPointerUp: (e) {
                    pointers.remove(e.pointer);
                    if (editor.drawing) unawaited(editor.end());
                    setState(() {});
                  },
                  onPointerCancel: (e) {
                    pointers.remove(e.pointer);
                    unawaited(editor.cancel());
                    setState(() {});
                  },
                  child: InteractiveViewer(
                    transformationController: transform,
                    constrained: false,
                    panEnabled: editor.tool == 'Pan' || pointers.length > 1,
                    scaleEnabled: true,
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
                          editor.tool,
                          Color(editor.color),
                          editor.selection,
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              if (!editor.zen)
                Positioned(
                  top: 12,
                  left: 14,
                  child: Text(
                    '${editor.width} × ${editor.height} px  /  RGBA',
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
  _CanvasPainter(
    this.image,
    this.grid,
    this.dark, [
    this.from,
    this.to,
    this.tool = '',
    this.preview = Colors.white,
    this.selection,
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
    if (image != null) {
      canvas.drawImageRect(
        image!,
        Rect.fromLTWH(0, 0, image!.width.toDouble(), image!.height.toDouble()),
        Offset.zero & size,
        Paint()..filterQuality = FilterQuality.none,
      );
    }
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
      old.grid != grid ||
      old.dark != dark ||
      old.from != from ||
      old.to != to ||
      old.tool != tool ||
      old.preview != preview ||
      old.selection != selection;
}
