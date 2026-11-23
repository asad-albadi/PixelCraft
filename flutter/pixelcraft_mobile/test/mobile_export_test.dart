import 'dart:isolate';
import 'dart:convert';
import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';

import 'editing_safety_test.dart' show DelayedNative;

class ExportNative extends DelayedNative {
  final requests = <int>[];
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
    requests.add(op);
    final result = response();
    if (op == 24) {
      result['project'] = TransferableTypedData.fromList([
        Uint8List.fromList([1, 2, 3]),
      ]);
    }
    return result;
  }
}

class PreparingNative extends ExportNative {
  final preparing = Completer<void>(), finishPreparation = Completer<void>();
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
    if (op == 28) {
      preparing.complete();
      await finishPreparation.future;
    }
    return super.send(
      action,
      op: op,
      a: a,
      b: b,
      input: input,
      phase: phase,
      color: color,
      size: size,
    );
  }
}

class SheetNative extends ExportNative {
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
    final result = await super.send(
      action,
      op: op,
      a: a,
      b: b,
      input: input,
      phase: phase,
      color: color,
      size: size,
    );
    if (op == 24 && a == 5) {
      result['project'] = TransferableTypedData.fromList([
        Uint8List.fromList(
          utf8.encode(
            jsonEncode({
              'meta': {'image': 'PixelCraft-sheet.png'},
              'frames': {},
            }),
          ),
        ),
      ]);
    }
    return result;
  }
}

class FlushingEditor extends EditorController {
  final flushing = Completer<void>(), releaseFlush = Completer<void>();
  FlushingEditor(ExportNative backend) : super(backend: backend);
  @override
  Future<void> flushRecovery() {
    if (!flushing.isCompleted) flushing.complete();
    return releaseFlush.future;
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('Export/share do not acknowledge saved baseline; cancellation leaves document dirty', () async {
    final backend = ExportNative()..dirty = true;
    final editor = EditorController(backend: backend);
    final calls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(EditorController.storage, (call) async {
          calls.add(call);
          return call.method == 'exportDocument' ? null : true;
        });
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(EditorController.storage, null),
    );
    await editor.command(2);
    await editor.exportArtwork(1);
    expect(editor.status, 'Export cancelled');
    expect(editor.dirty, true);
    expect(backend.requests.contains(12), false);
    await editor.exportArtwork(2, share: true);
    expect(calls.last.method, 'shareDocument');
    expect((calls.last.arguments as Map)['mime'], 'image/gif');
    expect(editor.dirty, true);
    editor.dispose();
    await Future<void>.delayed(const Duration(milliseconds: 150));
  });
  test('Import format detected by signature, with failed/unknown import preserving current target', () async {
    final backend = ExportNative();
    final editor = EditorController(backend: backend);
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          EditorController.storage,
          (call) async => {
            'name': 'renamed.bin',
            'bytes': Uint8List.fromList([137, 80, 78, 71, 13, 10, 26, 10]),
          },
        );
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(EditorController.storage, null),
    );
    await editor.command(2);
    await editor.open(picker: true);
    expect(backend.requests.contains(28), true);
    expect(backend.requests.last, 29);
    expect(editor.title, 'renamed.bin');
    editor.dispose();
    await Future<void>.delayed(const Duration(milliseconds: 150));
  });
  test('Cancel during staged import discards preparation without commit or history change', () async {
    final backend = PreparingNative()..dirty = true;
    final editor = EditorController(backend: backend);
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          EditorController.storage,
          (call) async => {
            'name': 'test.png',
            'bytes': Uint8List.fromList([137, 80, 78, 71, 13, 10, 26, 10]),
          },
        );
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(EditorController.storage, null),
    );
    await editor.command(2);
    final before = Map<String, dynamic>.from(editor.state);
    final open = editor.open(picker: true);
    await backend.preparing.future.timeout(const Duration(seconds: 2));
    editor.cancelImport();
    backend.finishPreparation.complete();
    await open;
    expect(backend.requests.contains(29), false);
    expect(backend.requests.contains(30), true);
    expect(editor.state, before);
    expect(editor.dirty, true);
    editor.dispose();
    await Future<void>.delayed(const Duration(milliseconds: 150));
  });
  test(
    'Repeated sheet export binds JSON to provider-disambiguated PNG name',
    () async {
      final backend = SheetNative();
      final editor = EditorController(backend: backend);
      final calls = <MethodCall>[];
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(EditorController.storage, (call) async {
            calls.add(call);
            return (call.arguments as Map?)?['returnName'] == true
                ? {'name': 'PixelCraft-sheet (1).png'}
                : true;
          });
      addTearDown(
        () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(EditorController.storage, null),
      );
      await editor.command(2);
      editor.exportFolder = 'Chosen folder';
      await editor.exportArtwork(4);
      expect(editor.error, isNull);
      await editor.exportArtwork(5);
      final args = calls.last.arguments as Map;
      final metadata = jsonDecode(utf8.decode(args['bytes'] as Uint8List));
      expect(metadata['meta']['image'], 'PixelCraft-sheet (1).png');
      expect(args['name'], 'PixelCraft-sheet (1).json');
      editor.dispose();
      await Future<void>.delayed(const Duration(milliseconds: 150));
    },
  );
  test(
    'Revoked folder grant reports failure without marking artwork saved',
    () async {
      final backend = ExportNative()..dirty = true;
      final editor = EditorController(backend: backend);
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(EditorController.storage, (call) async {
            if (call.method == 'savedExportFolder') return null;
            throw PlatformException(
              code: 'storage',
              message: 'Choose an export folder again; its write permission is unavailable',
            );
          });
      addTearDown(
        () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(EditorController.storage, null),
      );
      await editor.command(2);
      editor.exportFolder = 'Revoked folder';
      await editor.exportArtwork(1);
      expect(editor.error, contains('write permission'));
      expect(editor.dirty, true);
      expect(backend.requests.contains(12), false);
      await editor.loadExportFolder();
      expect(editor.exportFolder, isNull);
      editor.dispose();
      await Future<void>.delayed(const Duration(milliseconds: 150));
    },
  );
  test('Cancel also finishes a pending provider read before any native import starts', () async {
    final backend = ExportNative();
    final editor = EditorController(backend: backend);
    final reading = Completer<void>(), read = Completer<Object?>();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(EditorController.storage, (call) async {
          if (call.method == 'openDocument') {
            reading.complete();
            return read.future;
          }
          if (call.method == 'cancelOpenDocument') {
            read.complete(null);
            return null;
          }
          return null;
        });
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(EditorController.storage, null),
    );
    await editor.command(2);
    final before = Map<String, dynamic>.from(editor.state);
    final opening = editor.open(picker: true);
    await reading.future;
    editor.cancelImport();
    await opening.timeout(const Duration(milliseconds: 250));
    expect(editor.state, before);
    expect(editor.busy, false);
    expect(backend.requests.contains(28), false);
    editor.dispose();
    await Future<void>.delayed(const Duration(milliseconds: 150));
  });
  for (final kind in ['reference', 'export']) {
    test(
      '$kind rechecks availability after recovery without clearing another action',
      () async {
        final backend = ExportNative();
        final editor = FlushingEditor(backend);
        var calls = 0;
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(EditorController.storage, (call) async {
              calls++;
              return null;
            });
        addTearDown(
          () => TestDefaultBinaryMessengerBinding
              .instance
              .defaultBinaryMessenger
              .setMockMethodCallHandler(EditorController.storage, null),
        );
        await editor.command(2);
        final operation = kind == 'reference'
            ? editor.importReference()
            : editor.exportArtwork(1);
        await editor.flushing.future;
        editor.busy = true;
        editor.releaseFlush.complete();
        await operation;
        expect(calls, 0);
        expect(editor.busy, true);
        expect(backend.requests, [2]);
        editor.busy = false;
        editor.dispose();
        await Future<void>.delayed(const Duration(milliseconds: 150));
      },
    );
  }
}
