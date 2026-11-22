import 'dart:async';
import 'dart:isolate';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/bridge/native_editor.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';

class DelayedNative extends NativeEditor {
  final started = Completer<void>();
  final release = Completer<void>();
  final operations = <int>[];
  bool dirty = false;
  Map<String, dynamic> response() => {
    'state': {
      'width': 1,
      'height': 1,
      'frames': 1,
      'frame': 0,
      'durations': [100],
      'layer': 0,
      'layers': [],
      'undo': dirty ? 1 : 0,
      'redo': 0,
      'dirty': dirty,
      'revision': 1,
    },
    'rgba': TransferableTypedData.fromList([Uint8List(4)]),
  };
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
    if (action == 'stroke' && phase == 0) {
      started.complete();
      await release.future;
      dirty = true;
    }
    if (action == 'request') {
      operations.add(op);
      if (op == 0) dirty = false;
    }
    return response();
  }

  @override
  Future<void> close() async {}
}

void main() {
  testWidgets('Android back protects dirty document', (tester) async {
    final backend = DelayedNative()..dirty = true;
    final editor = EditorController(backend: backend);
    unawaited(editor.command(2));
    await tester.pump();
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump();
    expect(editor.ready, true);
    expect(editor.dirty, true);
    await tester.pumpWidget(PixelCraftApp(controller: editor));
    var exited = false;
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'SystemNavigator.pop') exited = true;
        return null;
      },
    );
    unawaited(tester.binding.handlePopRoute());
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 500));
    expect(exited, false);
    expect(find.text('Keep your changes?'), findsOneWidget);
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    );
    await tester.pumpWidget(const SizedBox());
    editor.dispose();
  });
  testWidgets('new document is disabled while stroke completes', (
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
    expect(editor.error, isNull);
    expect(editor.ready, true);
    await tester.pumpWidget(PixelCraftApp(controller: editor));
    await tester.pump();
    final canvas = find.byKey(const Key('pixel-canvas'));
    final viewport = tester.getRect(canvas);
    final viewer = tester.widget<InteractiveViewer>(
      find.byType(InteractiveViewer),
    );
    final zoom = Matrix4.identity()..scaleByDouble(2, 2, 1, 1);
    viewer.transformationController!.value = zoom;
    editor.begin(Offset.zero);
    unawaited(editor.end());
    await tester.pump();
    expect(backend.started.isCompleted, true);
    expect(editor.dirty, false);
    expect(editor.drawing, false);
    expect(editor.busy, true);
    expect(
      find.byType(LinearProgressIndicator),
      findsNothing,
      reason: 'Drawing must not show application-wide file loading',
    );
    expect(
      tester.getRect(canvas),
      viewport,
      reason: 'Pending work must not resize the canvas',
    );
    expect(
      viewer.transformationController!.value,
      zoom,
      reason: 'Drawing must preserve zoom',
    );
    await tester.tap(find.byTooltip('File actions'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 500));
    expect(
      tester
          .widget<PopupMenuItem<String>>(
            find.ancestor(
              of: find.text('New 64 × 64 sprite'),
              matching: find.byType(PopupMenuItem<String>),
            ),
          )
          .enabled,
      false,
    );
    expect(find.text('Keep your changes?'), findsNothing);
    backend.release.complete();
    await tester.pump();
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump();
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump();
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(backend.operations, isNot(contains(0)));
    expect(editor.dirty, true);
    await tester.pumpWidget(const SizedBox());
    editor.dispose();
  });
}
