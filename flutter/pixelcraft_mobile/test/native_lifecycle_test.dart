import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/bridge/native_editor.dart';

void main() {
  test(
    'Failed native startup rejects later commands without hanging',
    () async {
      final editor = NativeEditor();
      await expectLater(
        editor.start(library: '/missing/pixelcraft-test-library.so'),
        throwsStateError,
      );
      await expectLater(
        editor.send('request').timeout(const Duration(seconds: 1)),
        throwsStateError,
      );
      await editor.close();
    },
  );
}
