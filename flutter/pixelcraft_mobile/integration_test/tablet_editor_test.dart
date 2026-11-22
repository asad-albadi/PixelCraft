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
      addTearDown(() async {
        editor.dispose();
        await editor.native.close();
      });
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
      await tester.runAsync(() async {
        await EditorController.storage.invokeMethod<void>('recoveryClear');
        await editor.initialize();
      });
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      await finish();
      expect(editor.ready, isTrue);
      await tester.runAsync(() => editor.newDocument());
      await finish();
      final original = await pixels();
      final canvas = find.byKey(const Key('pixel-canvas'));
      final center = tester.getCenter(canvas);
      expect(
        editor.busy,
        isFalse,
        reason: 'Canvas must be interactive before drawing',
      );
      expect(editor.width, 64);
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

      await tester.runAsync(() => editor.flushRecovery());
      expect(
        await tester.runAsync(
          () => EditorController.storage.invokeMethod('recoveryRead'),
        ),
        isNotNull,
      );
      await finish();
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
  testWidgets(
    'Unsaved artwork survives an editor restart and retains its dirty baseline',
    (tester) async {
      final first = EditorController();
      await tester.runAsync(() async {
        await EditorController.storage.invokeMethod<void>('recoveryClear');
        await first.initialize();
        await first.newDocument();
        first.begin(const Offset(12, 13));
        await first.end();
        await first.pause();
      });
      expect(first.error, isNull);
      final expected = (await tester.runAsync(
        () async =>
            (await first.image!.toByteData(format: ui.ImageByteFormat.rawRgba))!
                .buffer
                .asUint8List(),
      ))!;
      await tester.runAsync(() => first.native.close());
      first.dispose();
      final restored = EditorController();
      await tester.runAsync(() => restored.initialize());
      expect(restored.recoveryPending, isTrue);
      await tester.pumpWidget(PixelCraftApp(controller: restored));
      await tester.pump();
      expect(find.text('Recover artwork'), findsOneWidget);
      await tester.runAsync(() => restored.resolveRecovery(restore: true));
      await tester.pump(const Duration(milliseconds: 150));
      expect(restored.error, isNull);
      expect(restored.dirty, isTrue);
      final actual = (await tester.runAsync(
        () async => (await restored.image!.toByteData(
          format: ui.ImageByteFormat.rawRgba,
        ))!.buffer.asUint8List(),
      ))!;
      expect(actual, orderedEquals(expected));
      await tester.runAsync(() => restored.save());
      await tester.runAsync(() => restored.flushRecovery());
      expect(
        await tester.runAsync(
          () => EditorController.storage.invokeMethod('recoveryRead'),
        ),
        isNull,
      );
      await tester.pumpWidget(const SizedBox());
      restored.dispose();
    },
  );
  testWidgets(
    'Shapes, fill, sampling, brushes and named colors use the Rust engine',
    (tester) async {
      final editor = EditorController();
      addTearDown(() async {
        editor.dispose();
        await editor.native.close();
      });
      await tester.runAsync(() async {
        await EditorController.storage.invokeMethod<void>('recoveryClear');
        await editor.initialize();
        await editor.newDocument();
        await editor.addNamedColor('Ocean', 0xff2060a0);
        editor.setTool('Rectangle');
        editor.begin(const Offset(4, 4));
        editor.move(const Offset(12, 12));
        await editor.end();
        editor.setColor(0xffffcc40);
        editor.setTool('Fill');
        editor.begin(const Offset(6, 6));
        await editor.end();
        editor.setColor(0xff000000);
        editor.setTool('Eyedropper');
        editor.begin(const Offset(6, 6));
        await editor.end();
        expect(editor.color, 0xffffcc40);
        final undo = editor.state['undo'];
        await editor.setBrushShape(1);
        expect(editor.state['undo'], undo);
        editor.setTool('Line');
        editor.begin(const Offset(20, 20));
        editor.move(const Offset(30, 30));
        await editor.end();
        editor.setTool('Ellipse');
        editor.begin(const Offset(40, 40));
        editor.move(const Offset(50, 50));
        await editor.end();
        final committed = editor.state['undo'];
        editor.setTool('Rectangle');
        editor.begin(const Offset(2, 20));
        await editor.cancel();
        expect(editor.state['undo'], committed);
        expect(editor.error, isNull);
        expect(editor.namedColors.any((c) => c.$1 == 'Ocean'), isTrue);
        await editor.save();
        await editor.newDocument();
        await editor.open();
        expect(
          editor.namedColors.any((c) => c.$1 == 'Ocean' && c.$2 == 0xff2060a0),
          isTrue,
        );
      });
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      await tester.pump(const Duration(milliseconds: 150));
      expect(find.byTooltip('Fill'), findsOneWidget);
      expect(find.byTooltip('Rectangle'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
    },
  );
  testWidgets(
    'Selections, layers, linked cels, frame names, tags and playback stay editable',
    (tester) async {
      final editor = EditorController();
      addTearDown(() async {
        editor.dispose();
        await editor.native.close();
      });
      await tester.runAsync(() async {
        await EditorController.storage.invokeMethod<void>('recoveryClear');
        await editor.initialize();
        await editor.newDocument();
        editor.begin(const Offset(5, 5));
        await editor.end();
        await editor.selectionAction(
          'select',
          extra: {
            'bounds': [2, 2, 8, 8],
          },
        );
        await editor.selectionAction('copy');
        await editor.selectionAction('paste', extra: {'x': 15, 'y': 15});
        await editor.selectionAction('flip-x');
        await editor.selectionAction('rotate');
        await editor.selectionAction('crop');
        expect(editor.width, 7);
        expect(editor.height, 7);
        await editor.addLayer();
        expect(editor.state['layers'].length, 2);
        expect(editor.state['layer'], 1);
        editor.begin(const Offset(1, 1));
        await editor.end();
        await editor.addFrame();
        expect(editor.state['frame'], 1);
        await editor.edits([
          {'op': 'link-cel', 'source_frame': 0},
        ]);
        expect(editor.state['layers'][1]['linked'], 0);
        await editor.edits([
          {'op': 'set-frame', 'name': 'attack_01', 'duration': 80},
        ]);
        await editor.edits([
          {
            'op': 'add-tag',
            'tag': {
              'name': 'attack',
              'from': 0,
              'to': 1,
              'direction': 0,
              'repeat': 1,
              'color': [64, 200, 180],
            },
          },
        ]);
        final target = editor.state['frame'], history = editor.state['undo'];
        await editor.startPlayback(tag: 0);
        await Future<void>.delayed(const Duration(milliseconds: 600));
        await editor.stopPlayback();
        expect(editor.state['frame'], target);
        expect(editor.state['undo'], history);
        expect(editor.error, isNull);
        await editor.save();
        await editor.newDocument();
        await editor.open();
        expect(editor.state['tags'][0]['name'], 'attack');
        expect(editor.state['frame_names'][1], 'attack_01');
        expect(editor.state['durations'][1], 80);
      });
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      await tester.pump(const Duration(milliseconds: 150));
      await tester.longPress(find.text('2'));
      await tester.runAsync(() => editor.settle());
      await tester.pump(const Duration(milliseconds: 500));
      expect(editor.state['frame'], 1);
      expect(find.text('Frame 2 of 2'), findsOneWidget);
      expect(find.text('Frame duration'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
    },
  );
  testWidgets(
    'Zen supports real drawing, Undo and return to normal on Android',
    (tester) async {
      final editor = EditorController();
      addTearDown(() async {
        editor.dispose();
        await editor.native.close();
      });
      await tester.runAsync(() async {
        await EditorController.storage.invokeMethod<void>('recoveryClear');
        await editor.initialize();
        await editor.newDocument();
      });
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      await tester.pump(const Duration(milliseconds: 150));
      await tester.tap(find.byTooltip('Zen drawing mode'));
      await tester.pump(const Duration(milliseconds: 150));
      expect(editor.zen, true);
      expect(find.text('Preview'), findsNothing);
      final gesture = await tester.startGesture(
        tester.getCenter(find.byKey(const Key('pixel-canvas'))),
      );
      await gesture.moveBy(const Offset(20, 10));
      await gesture.up();
      await tester.runAsync(() => editor.settle());
      await tester.pump(const Duration(milliseconds: 150));
      expect(editor.state['undo'], 1);
      expect(editor.error, isNull);
      await tester.tap(find.byTooltip('Undo'));
      await tester.runAsync(() => editor.settle());
      await tester.pump(const Duration(milliseconds: 150));
      expect(editor.state['undo'], 0);
      await tester.tap(find.byKey(const Key('exit-zen')));
      await tester.pump(const Duration(milliseconds: 150));
      expect(editor.zen, false);
      expect(find.text('Preview'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
