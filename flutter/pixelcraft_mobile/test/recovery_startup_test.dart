import 'dart:async';
import 'dart:isolate';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/bridge/native_editor.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/storage/recovery_store.dart';

class StartupNative extends NativeEditor {
  @override
  Future<void> start({String library = ''}) async {}
  @override
  Future<void> close() async {}
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
  }) async => {
    'state': {
      'width': 1,
      'height': 1,
      'frames': 1,
      'frame': 0,
      'layer': 0,
      'layers': [],
      'durations': [100],
      'revision': 1,
      'dirty': false,
      'undo': 0,
      'redo': 0,
    },
    'rgba': TransferableTypedData.fromList([Uint8List(4)]),
  };
}

class DelayedRecovery extends RecoveryStore {
  final pending = Completer<Uint8List?>();
  final reading = Completer<void>();
  var clears = 0;
  @override
  Future<Uint8List?> read() {
    reading.complete();
    return pending.future;
  }

  @override
  Future<void> clear() async {
    clears++;
  }
}

void main() {
  testWidgets(
    'Startup blocks drawing and background flush until recovery is read',
    (tester) async {
      final store = DelayedRecovery();
      final editor = EditorController(
        backend: StartupNative(),
        recoveryStore: store,
      );
      unawaited(editor.initialize());
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump();
      expect(store.reading.isCompleted, isTrue);
      expect(editor.ready, isTrue);
      expect(editor.busy, isTrue);
      editor.begin(Offset.zero);
      expect(editor.drawing, isFalse);
      final paused = editor.pause();
      await tester.pump();
      await paused;
      expect(store.clears, 0);
      store.pending.complete(Uint8List.fromList([1, 2, 3]));
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump();
      expect(editor.recoveryPending, isTrue);
      expect(editor.busy, isTrue);
      final resolved = editor.resolveRecovery(restore: false);
      await tester.pump();
      await resolved;
      expect(store.clears, 1);
      expect(editor.recoveryPending, isFalse);
      editor.dispose();
      await tester.pump(const Duration(milliseconds: 150));
    },
  );
}
