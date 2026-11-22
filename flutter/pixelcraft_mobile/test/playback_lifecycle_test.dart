import 'dart:async';
import 'dart:isolate';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/bridge/native_editor.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/storage/recovery_store.dart';

class PlaybackNative extends NativeEditor {
  final metadata = Completer<void>(), release = Completer<void>();
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
    if (op == 22) {
      metadata.complete();
      await release.future;
    }
    return {
      'state': {
        'width': 1,
        'height': 1,
        'frames': 2,
        'frame': 0,
        'layer': 0,
        'layers': [],
        'durations': [100, 100],
        'revision': 1,
        'dirty': true,
        'undo': 1,
        'redo': 0,
      },
      'rgba': TransferableTypedData.fromList([Uint8List(4)]),
      if (op == 15)
        'recovery': TransferableTypedData.fromList([
          Uint8List.fromList([1, 2, 3]),
        ]),
      if (op == 22)
        'value': {
          'sequence': [0, 1],
          'durations': [100, 100],
          'repeat': 0,
        },
    };
  }
}

class MemoryRecovery extends RecoveryStore {
  int writes = 0;
  @override
  Future<void> write(Uint8List bytes) async {
    writes++;
  }

  @override
  Future<void> clear() async {}
}

void main() {
  testWidgets(
    'Playback saves pending artwork first and pause cancels a pending start',
    (tester) async {
      final native = PlaybackNative(), store = MemoryRecovery();
      final e = EditorController(backend: native, recoveryStore: store);
      unawaited(e.command(2));
      for (var i = 0; i < 5 && !e.ready; i++) {
        await tester.pump();
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 20)),
        );
      }
      expect(e.ready, true);
      bool started = false, paused = false;
      unawaited(
        e.startPlayback().then((_) {
          started = true;
        }),
      );
      for (var i = 0; i < 20 && !native.metadata.isCompleted; i++) {
        await tester.pump();
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 20)),
        );
      }
      expect(native.metadata.isCompleted, true);
      expect(
        store.writes,
        1,
        reason: 'Unsaved artwork must be protected before looping playback',
      );
      unawaited(
        e.pause().then((_) {
          paused = true;
        }),
      );
      native.release.complete();
      for (var i = 0; i < 20 && (!started || !paused); i++) {
        await tester.pump();
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 20)),
        );
      }
      expect(started, true);
      expect(paused, true);
      expect(e.playing, false);
      expect(e.busy, false);
      e.dispose();
      await tester.pump(const Duration(milliseconds: 150));
    },
  );
}
