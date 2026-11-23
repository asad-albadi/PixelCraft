import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/storage/recovery_store.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test(
    'Recovery writes and discard stay ordered and propagate storage errors',
    () async {
      const channel = MethodChannel('pixelcraft/test-recovery');
      final events = <String>[];
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            events.add(call.method);
            if (call.method == 'recoveryWrite' &&
                (call.arguments as Map)['bytes'][0] == 0) {
              throw PlatformException(
                code: 'disk-full',
                message: 'Storage full',
              );
            }
            if (call.method == 'recoveryRead') {
              return Uint8List.fromList([1, 2, 3]);
            }
            return null;
          });
      final store = RecoveryStore(channel: channel);
      final first = store.write(Uint8List.fromList([1]));
      final clear = store.clear();
      await first;
      await clear;
      expect(events, ['recoveryWrite', 'recoveryClear']);
      expect(await store.read(), orderedEquals([1, 2, 3]));
      await expectLater(
        store.write(Uint8List.fromList([0])),
        throwsA(isA<PlatformException>()),
      );
      await store.write(Uint8List.fromList([2]));
      expect(events.last, 'recoveryWrite');
    },
  );
}
