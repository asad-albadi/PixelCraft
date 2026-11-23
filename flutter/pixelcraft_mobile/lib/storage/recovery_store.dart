import 'package:flutter/services.dart';

/// Serialize publication and discard so an old write cannot resurrect a cleared draft.
class RecoveryStore {
  final MethodChannel channel;
  Future<void> _tail = Future.value();
  RecoveryStore({this.channel = const MethodChannel('pixelcraft/storage')});
  Future<T> _ordered<T>(Future<T> Function() work) {
    final next = _tail.then((_) => work());
    _tail = next.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return next;
  }

  Future<Uint8List?> read() =>
      _ordered(() => channel.invokeMethod<Uint8List>('recoveryRead'));
  Future<void> write(Uint8List bytes) => _ordered(
    () => channel.invokeMethod<void>('recoveryWrite', {'bytes': bytes}),
  );
  Future<void> clear() =>
      _ordered(() => channel.invokeMethod<void>('recoveryClear'));
}
