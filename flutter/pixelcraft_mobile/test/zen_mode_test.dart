import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';

import 'editing_safety_test.dart' show DelayedNative;

void main() {
  testWidgets(
    'Zen preserves artwork/view and Back restores editor without exiting',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final editor = EditorController(backend: DelayedNative());
      unawaited(editor.command(2));
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump();
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      await tester.pump(const Duration(milliseconds: 150));
      final doc = Map<String, dynamic>.from(editor.state);
      final matrix = Matrix4.identity()
        ..translateByDouble(23, 19, 0, 1)
        ..scaleByDouble(2, 2, 1, 1);
      editor.canvasTransform.value = matrix;
      editor.togglePixelGrid();
      editor.setZen(true);
      await tester.pump();
      await tester.pump();
      expect(find.byKey(const Key('exit-zen')), findsOneWidget);
      expect(find.text('Preview'), findsNothing);
      expect(editor.canvasTransform.value.storage[0], 2);
      expect(editor.state, doc);
      unawaited(tester.binding.handlePopRoute());
      await tester.pump();
      await tester.pump();
      expect(editor.zen, false);
      expect(editor.pixelGrid, true);
      expect(find.text('Preview'), findsOneWidget);
      for (var i = 0; i < 16; i++) {
        expect(
          editor.canvasTransform.value.storage[i],
          closeTo(matrix.storage[i], .000001),
        );
      }
      expect(editor.state, doc);
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
      await tester.pump(const Duration(milliseconds: 150));
    },
  );
  for (final size in [const Size(390, 844), const Size(800, 360)]) {
    testWidgets('Zen controls remain reachable at $size with large text', (
      tester,
    ) async {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final backend = DelayedNative();
      final editor = EditorController(backend: backend);
      unawaited(editor.command(2));
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump();
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      editor.setZen(true);
      await tester.pump();
      await tester.pumpWidget(
        MediaQuery(
          data: MediaQueryData(
            size: size,
            textScaler: const TextScaler.linear(2),
          ),
          child: PixelCraftApp(controller: editor),
        ),
      );
      await tester.pump();
      expect(tester.takeException(), isNull);
      final exit = find.byKey(const Key('exit-zen'));
      expect(exit, findsOneWidget);
      await tester.tap(find.byTooltip('Eraser'));
      await tester.pump();
      expect(editor.tool, 'Erase');
      expect(
        backend.started.isCompleted,
        false,
        reason: 'toolbar must not paint through to canvas',
      );
      await tester.tap(exit);
      await tester.pump();
      expect(editor.zen, false);
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
      await tester.pump(const Duration(milliseconds: 150));
    });
  }
  testWidgets('Exit Zen cancels an active shape before switching canvas', (
    tester,
  ) async {
    final backend = DelayedNative();
    final editor = EditorController(backend: backend);
    unawaited(editor.command(2));
    await tester.pump();
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump();
    await tester.pumpWidget(PixelCraftApp(controller: editor));
    editor.setZen(true);
    editor.setTool('Rectangle');
    await tester.pump(const Duration(milliseconds: 150));
    editor.begin(Offset.zero);
    expect(editor.drawing, true);
    await tester.tap(find.byKey(const Key('exit-zen')));
    await tester.pump();
    expect(editor.zen, false);
    expect(editor.drawing, false);
    expect(editor.gestureStart, isNull);
    expect(backend.operations, [2]);
    await tester.pumpWidget(const SizedBox());
    editor.dispose();
    await tester.pump(const Duration(milliseconds: 150));
  });
}
