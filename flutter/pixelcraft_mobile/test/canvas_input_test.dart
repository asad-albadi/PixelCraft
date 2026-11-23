import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';

import 'editing_safety_test.dart' show DelayedNative;

class InputNative extends DelayedNative {
  final strokes = <int>[];
  final sizes = <int>[];
  @override
  Future<Map<String, dynamic>> send(
    String action, {
    int op = 2,
    int a = 0,
    int b = 0,
    Uint8List? input,
    int phase = 0,
    int color = 0,
    int size = 1,
  }) async {
    if (action == 'stroke' && phase == 0) strokes.add(color);
    if (action == 'stroke') sizes.add(size);
    return response();
  }
}

void main() {
  testWidgets(
    'Secondary mouse button cannot paint; inverted stylus erases with selected tool preserved',
    (tester) async {
      final native = InputNative();
      final editor = EditorController(backend: native);
      unawaited(editor.command(2));
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump();
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      await tester.pump(const Duration(milliseconds: 150));
      final center = tester.getCenter(find.byKey(const Key('pixel-canvas')));
      final mouse = await tester.startGesture(
        center,
        kind: ui.PointerDeviceKind.mouse,
        buttons: kSecondaryMouseButton,
      );
      await mouse.up();
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump(const Duration(milliseconds: 150));
      expect(native.strokes, isEmpty);
      editor.setTool('Rectangle');
      final pen = await tester.startGesture(
        center,
        kind: ui.PointerDeviceKind.invertedStylus,
      );
      await pen.up();
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump(const Duration(milliseconds: 150));
      expect(native.strokes, [0]);
      expect(editor.tool, 'Rectangle');
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
      await tester.pump(const Duration(milliseconds: 150));
    },
  );
  testWidgets(
    'Optional pressure maps bounded brush size without changing selected size',
    (tester) async {
      final native = InputNative();
      final editor = EditorController(backend: native);
      unawaited(editor.command(2));
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump();
      editor.setBrush(8);
      editor.setStylusPressure(true);
      editor.begin(Offset.zero, pressure: 0);
      editor.move(Offset.zero, pressure: 1);
      unawaited(editor.end());
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump(const Duration(milliseconds: 150));
      expect(native.sizes.first, 2);
      expect(native.sizes.last, 8);
      expect(editor.brush, 8);
      editor.dispose();
      await tester.pump(const Duration(milliseconds: 150));
    },
  );
}
