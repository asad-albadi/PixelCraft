import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';

void main() {
  for (final width in [1280.0, 390.0]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets(
        'Colors and file controls remain reachable at $width and text scale $scale',
        (tester) async {
          tester.view.physicalSize = Size(width, 844);
          tester.view.devicePixelRatio = 1;
          tester.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
          final editor = EditorController();
          editor.state = {
            'width': 64,
            'height': 64,
            'frames': 1,
            'frame': 0,
            'durations': [100],
            'layer': 0,
            'layers': [
              {
                'name': 'Layer 1',
                'visible': true,
                'locked': false,
                'group': false,
              },
            ],
            'undo': 0,
            'redo': 0,
            'dirty': false,
          };
          await tester.pumpWidget(PixelCraftApp(controller: editor));
          await tester.pump();
          expect(tester.takeException(), isNull);
          expect(find.byTooltip('File actions'), findsOneWidget);
          expect(find.byKey(const Key('save')), findsOneWidget);
          if (width >= 850) {
            expect(find.text('Colors'), findsOneWidget);
            expect(find.text('Preview'), findsOneWidget);
          } else {
            await tester.tap(find.byTooltip('Colors and layers'));
            await tester.pump(const Duration(milliseconds: 500));
            expect(find.text('Colors'), findsOneWidget);
            expect(find.text('Preview'), findsOneWidget);
          }
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox());
          editor.dispose();
        },
      );
    }
  }
}
