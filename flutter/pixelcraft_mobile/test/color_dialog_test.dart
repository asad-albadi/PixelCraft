import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/widgets/color_dialog.dart';

void main() {
  Future<void> open(
    WidgetTester tester,
    void Function((String, int)?) result, {
    int initial = 0xff123456,
  }) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) {
            return Scaffold(
              body: TextButton(
                onPressed: () async {
                  result(await editColor(context, initial));
                },
                child: const Text('Open'),
              ),
            );
          },
        ),
      ),
    );
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
  }

  testWidgets('six RGB digits validate strictly and preserve separate alpha', (
    tester,
  ) async {
    (String, int)? picked;
    await open(tester, (value) => picked = value, initial: 0x80123456);
    final name = find.widgetWithText(TextField, 'Name');
    final hex = find.widgetWithText(TextField, 'RGB hex');
    await tester.enterText(name, '');
    await tester.pump();
    expect(
      tester
          .widget<FilledButton>(find.widgetWithText(FilledButton, 'Add'))
          .onPressed,
      isNull,
    );
    await tester.enterText(name, 'Ocean');
    for (final invalid in [
      '-FFFFF',
      '+FFFFF',
      ' FFFFF',
      'FFFFFF ',
      '80112233',
      '12345',
      'ZZZZZZ',
    ]) {
      await tester.enterText(hex, invalid);
      await tester.pump();
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Add'))
            .onPressed,
        isNull,
        reason: invalid,
      );
    }
    await tester.enterText(hex, '11aAbb');
    await tester.pump();
    await tester.ensureVisible(find.text('Add'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Add'));
    await tester.pumpAndSettle();
    expect(picked, ('Ocean', 0x8011aabb));
  });

  testWidgets('touch surface, hue and opacity determine returned ARGB', (
    tester,
  ) async {
    (String, int)? picked;
    await open(tester, (value) => picked = value, initial: 0xffff0000);
    final sv = find.byKey(const ValueKey('color-sv'));
    final hue = find.byKey(const ValueKey('color-hue'));
    final opacity = find.byKey(const ValueKey('color-opacity'));
    final hueRect = tester.getRect(hue);
    await tester.tapAt(
      Offset(hueRect.left + hueRect.width / 3, hueRect.center.dy),
    );
    await tester.pump();
    final svRect = tester.getRect(sv);
    await tester.tapAt(svRect.center);
    await tester.pump();
    final alphaRect = tester.getRect(opacity);
    await tester.tapAt(alphaRect.center);
    await tester.pump();
    await tester.ensureVisible(find.text('Add'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Add'));
    await tester.pumpAndSettle();
    final color = Color(picked!.$2);
    expect(color.a, closeTo(0.5, 0.01));
    expect(color.g, closeTo(0.5, 0.01));
    expect(color.r, closeTo(0.25, 0.01));
    expect(color.b, closeTo(0.25, 0.01));
  });

  testWidgets('keyboard adjusts focused opacity and grayscale retains hue', (
    tester,
  ) async {
    (String, int)? picked;
    await open(tester, (value) => picked = value, initial: 0xff00ff00);
    final hex = find.widgetWithText(TextField, 'RGB hex');
    await tester.ensureVisible(hex);
    await tester.enterText(hex, '808080');
    await tester.pump();
    final sv = find.byKey(const ValueKey('color-sv'));
    await tester.ensureVisible(sv);
    await tester.tapAt(tester.getRect(sv).center);
    await tester.pump();
    final alpha = find.byKey(const ValueKey('color-opacity'));
    await tester.ensureVisible(alpha);
    await tester.tapAt(tester.getRect(alpha).center);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pump();
    await tester.ensureVisible(find.text('Add'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Add'));
    await tester.pumpAndSettle();
    final color = Color(picked!.$2);
    expect(color.g, closeTo(0.5, 0.01));
    expect(color.r, closeTo(0.25, 0.01));
    expect(color.b, closeTo(0.25, 0.01));
    expect(color.a, closeTo(0.51, 0.004));
  });

  testWidgets(
    '390 by 844 with double text and keyboard scrolls without overflow',
    (tester) async {
      tester.view.physicalSize = const Size(390, 844);
      tester.view.devicePixelRatio = 1;
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      await open(tester, (_) {});
      expect(tester.takeException(), isNull);
      await tester.ensureVisible(find.widgetWithText(TextField, 'RGB hex'));
      await tester.tap(find.widgetWithText(TextField, 'RGB hex'));
      tester.view.viewInsets = const FakeViewPadding(bottom: 300);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.ensureVisible(find.text('Add'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      tester.view.resetViewInsets();
    },
  );
}
