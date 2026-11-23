import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';

import 'editing_safety_test.dart' show DelayedNative;

void main() {
  testWidgets(
    'Hardware keyboard selects tools and toggles Zen without changing document',
    (tester) async {
      final editor = EditorController(backend: DelayedNative());
      unawaited(editor.command(2));
      await tester.pump();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump();
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      await tester.pump(const Duration(milliseconds: 150));
      final before = Map<String, dynamic>.from(editor.state);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyE);
      await tester.pump();
      expect(editor.tool, 'Erase');
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyF);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(editor.zen, true);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(editor.zen, false);
      expect(editor.state, before);
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
      await tester.pump(const Duration(milliseconds: 150));
    },
  );
}
