import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';

void main() {
  for (final width in [390.0, 1280.0]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets(
        'Privacy policy is readable at $width with text scale $scale',
        (tester) async {
          tester.view.physicalSize = Size(width, 844);
          tester.view.devicePixelRatio = 1;
          tester.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          await tester.runAsync(
            () => rootBundle.loadString('assets/legal/privacy-policy.md'),
          );
          final editor = EditorController();
          await tester.pumpWidget(PixelCraftApp(controller: editor));
          await tester.pump();
          await tester.tap(find.byTooltip('File actions'));
          await tester.pump(const Duration(milliseconds: 400));
          expect(find.text('Privacy policy'), findsOneWidget);
          await tester.ensureVisible(find.text('Privacy policy'));
          await tester.pump(const Duration(milliseconds: 400));
          await tester.tap(find.text('Privacy policy'));
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 400));
          await tester.runAsync(
            () => Future<void>.delayed(const Duration(milliseconds: 100)),
          );
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 400));
          expect(find.text('Artwork and local storage'), findsOneWidget);
          expect(find.text('Changes and contact'), findsOneWidget);
          expect(find.byTooltip('Close privacy policy'), findsOneWidget);
          expect(tester.takeException(), isNull);
          await tester.tap(find.byTooltip('Close privacy policy'));
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 400));
          expect(find.text('Artwork and local storage'), findsNothing);
          expect(find.byTooltip('File actions'), findsOneWidget);
          await tester.pumpWidget(const SizedBox());
          editor.dispose();
        },
      );
    }
  }

  test('Offline Android privacy policy matches the repository policy', () {
    expect(
      File('assets/legal/privacy-policy.md').readAsStringSync(),
      File('../../PRIVACY.md').readAsStringSync(),
    );
  });
}
