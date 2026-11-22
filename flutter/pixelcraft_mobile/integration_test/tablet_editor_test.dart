import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets(
    'Shared engine draws, cancels multitouch, undoes and durably reopens on Android',
    (tester) async {
      final editor = EditorController();
      Future<void> finish() async {
        await tester.runAsync(() => editor.settle());
        await tester.pump(const Duration(milliseconds: 150));
        expect(editor.error, isNull);
        expect(tester.takeException(), isNull);
      }

      Future<Uint8List> pixels() async => (await tester.runAsync(
        () async => (await editor.image!.toByteData(
          format: ui.ImageByteFormat.rawRgba,
        ))!.buffer.asUint8List(),
      ))!;
      await tester.runAsync(() => editor.initialize());
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      await finish();
      expect(editor.ready, isTrue);
      await tester.runAsync(() => editor.newDocument());
      await finish();
      final original = await pixels();
      final canvas = find.byKey(const Key('pixel-canvas'));
      final center = tester.getCenter(canvas);
      final gesture = await tester.startGesture(center);
      await tester.pump();
      await gesture.moveBy(const Offset(40, 24));
      await gesture.up();
      await finish();
      final drawn = await pixels();
      expect(drawn, isNot(orderedEquals(original)));
      expect(editor.state['undo'], 1);
      expect(editor.dirty, isTrue);
      await tester.tap(find.byTooltip('Undo'));
      await finish();
      expect(await pixels(), orderedEquals(original));
      await tester.tap(find.byTooltip('Redo'));
      await finish();
      expect(await pixels(), orderedEquals(drawn));

      final first = await tester.startGesture(
        center + const Offset(-40, -20),
        pointer: 11,
      );
      await tester.pump();
      final second = await tester.startGesture(
        center + const Offset(40, 20),
        pointer: 12,
      );
      await tester.pump();
      await first.up();
      await second.up();
      await finish();
      expect(
        await pixels(),
        orderedEquals(drawn),
        reason: 'Second finger cancels the provisional stroke',
      );
      expect(editor.state['undo'], 1);

      await tester.tap(find.byKey(const Key('save')));
      await finish();
      // Native file I/O completes outside the controller's editing queue.
      await tester.runAsync(() async {
        final until = DateTime.now().add(const Duration(seconds: 20));
        while (editor.busy && DateTime.now().isBefore(until)) {
          await Future<void>.delayed(const Duration(milliseconds: 20));
        }
      });
      await finish();
      expect(editor.busy, isFalse);
      expect(editor.savedKey, isNotNull);
      expect(editor.dirty, isFalse);
      final key = editor.savedKey;
      await tester.runAsync(() => editor.newDocument());
      await finish();
      await tester.runAsync(() => editor.open());
      await finish();
      expect(editor.savedKey, key);
      expect(await pixels(), orderedEquals(drawn));
      expect(editor.dirty, isFalse);

      await tester.runAsync(() async {
        await expectLater(
          EditorController.storage.invokeMethod('save', {
            'bytes': Uint8List(1),
            'key': '../../outside',
          }),
          throwsA(isA<PlatformException>()),
        );
        await EditorController.storage.invokeMethod('save', {
          'bytes': Uint8List.fromList([1, 2, 3]),
          'key': '00000000-0000-0000-0000-000000000029',
        });
        editor.savedKey = '00000000-0000-0000-0000-000000000029';
        await editor.open();
      });
      await tester.pump();
      expect(editor.error, isNotNull);
      expect(
        await pixels(),
        orderedEquals(drawn),
        reason: 'Malformed project must not replace the current artwork',
      );
      editor.savedKey = key;
      editor.dismissError();
      await tester.runAsync(() => editor.save());
      await finish();
      await tester.tap(find.byTooltip('Change appearance'));
      await tester.pump(const Duration(milliseconds: 300));
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
    },
  );
}
