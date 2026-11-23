import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';

const links = MethodChannel('pixelcraft/links');

void main() {
  for (final width in [390.0, 1280.0]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets(
        'Privacy policy opens browser at $width with text scale $scale',
        (tester) async {
          final calls = <MethodCall>[];
          tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
            links,
            (call) async {
              calls.add(call);
              return null;
            },
          );
          addTearDown(
            () => tester.binding.defaultBinaryMessenger
                .setMockMethodCallHandler(links, null),
          );
          tester.view.physicalSize = Size(width, 844);
          tester.view.devicePixelRatio = 1;
          tester.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          final editor = EditorController();
          await tester.pumpWidget(PixelCraftApp(controller: editor));
          await tester.pump();
          await tester.tap(find.byTooltip('File actions'));
          await tester.pump(const Duration(milliseconds: 400));
          await tester.ensureVisible(find.text('Privacy policy'));
          await tester.pump(const Duration(milliseconds: 400));
          await tester.tap(find.text('Privacy policy'));
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 400));
          expect(calls, hasLength(1));
          expect(calls.single.method, 'privacyPolicy');
          expect(calls.single.arguments, isNull);
          expect(find.text('Artwork and local storage'), findsNothing);
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox());
          editor.dispose();
        },
      );
    }
  }

  testWidgets('Browser launch failure is reported without losing the editor', (
    tester,
  ) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      links,
      (call) async => throw PlatformException(code: 'browser'),
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        links,
        null,
      ),
    );
    final editor = EditorController();
    await tester.pumpWidget(PixelCraftApp(controller: editor));
    await tester.pump();
    await tester.tap(find.byTooltip('File actions'));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.ensureVisible(find.text('Privacy policy'));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.tap(find.text('Privacy policy'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(
      find.text('Could not open the browser. Please try again.'),
      findsOneWidget,
    );
    expect(find.byTooltip('File actions'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    editor.dispose();
  });
}
