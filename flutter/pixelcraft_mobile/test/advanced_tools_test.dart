import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/widgets/advanced_tools.dart';

import 'timeline_tools_test.dart' show TargetEditor, fixture;

void main() {
  for (final size in [const Size(390, 844), const Size(800, 360)]) {
    testWidgets('Advanced tools stay scrollable at $size with large text', (
      tester,
    ) async {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1;
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      final editor = TargetEditor()..state = fixture().state;
      editor.state['tilesets'] = [
        {
          'index': 0,
          'name': 'Garden tiles',
          'width': 8,
          'height': 8,
          'count': 36,
        },
      ];
      editor.state['references'] = [
        {
          'name': 'Reference',
          'width': 48,
          'height': 48,
          'index': 0,
          'x': 0,
          'y': 0,
          'scale': 1,
          'scale_y': null,
          'opacity': 255,
          'visible': true,
          'locked': false,
        },
      ];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: AdvancedTools(editor: editor)),
        ),
      );
      await tester.pump();
      expect(find.text('Color mode: Rgb'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.scrollUntilVisible(
        find.byTooltip('Reference options 1'),
        400,
      );
      await tester.tap(find.byTooltip('Reference options 1'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Position & opacity…'));
      await tester.pumpAndSettle();
      expect(find.text('Reference position'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
    });
  }
}
