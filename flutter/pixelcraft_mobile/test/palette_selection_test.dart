import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';

void main() {
  testWidgets('selected named color removed by undo', (tester) async {
    final editor = EditorController();
    editor.state = {
      'palette': [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
      ],
      'palette_names': ['First', 'Last'],
    };
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: Inspector(editor: editor)),
      ),
    );
    await tester.tap(find.byType(DropdownButton<int>));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 500));
    await tester.tap(find.text('Last').last);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 500));
    editor.state = {
      'palette': [
        [255, 0, 0, 255],
      ],
      'palette_names': ['First'],
    };
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: Inspector(editor: editor)),
      ),
    );
    await tester.pump();
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    editor.dispose();
  });
}
