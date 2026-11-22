import 'dart:async';
import 'dart:isolate';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'bridge/native_editor.dart';

class EditorController extends ChangeNotifier {
  final NativeEditor native;
  static const storage = MethodChannel('pixelcraft/storage');
  Map<String, dynamic> state = {};
  ui.Image? image;
  String status = 'Opening your canvas…';
  String? error;
  String title = 'Moonlit garden';
  String? savedKey;
  bool _fileBusy = false;
  int _pendingActions = 0;
  bool get busy => _fileBusy || _pendingActions > 0;
  set busy(bool value) => _fileBusy = value;
  bool drawing = false;
  bool _disposed = false;
  bool _flushing = false;
  Timer? _timer;
  final List<Offset> _points = [];
  Future<void> _queue = Future.value();
  int color = 0xfff7e6b6;
  int brush = 1;
  String tool = 'Draw';
  int documentGeneration = 0;
  EditorController({NativeEditor? backend})
    : native = backend ?? NativeEditor();
  int get width => (state['width'] as num?)?.toInt() ?? 48;
  int get height => (state['height'] as num?)?.toInt() ?? 48;
  bool get dirty => state['dirty'] == true;
  bool get ready => state.isNotEmpty && image != null;
  bool get canUndo =>
      ready && !busy && !drawing && (state['undo'] as num).toInt() > 0;
  bool get canRedo =>
      ready && !busy && !drawing && (state['redo'] as num).toInt() > 0;
  void dismissError() {
    error = null;
    _notify();
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  Future<void> initialize() async {
    try {
      await native.start();
      await command(1);
    } catch (e) {
      error = e.toString();
      status = 'Could not open the editor';
      _notify();
    }
  }

  Future<Map<String, dynamic>?> _enqueue(
    Future<Map<String, dynamic>> Function() action,
  ) {
    final complete = Completer<Map<String, dynamic>?>();
    _pendingActions++;
    _notify();
    _queue = _queue
        .then((_) async {
          if (_disposed) {
            complete.complete(null);
            return;
          }
          try {
            final result = await action();
            if (_disposed) {
              complete.complete(null);
              return;
            }
            await _apply(result);
            error = null;
            _notify();
            complete.complete(result);
          } catch (e) {
            final message = e.toString().replaceFirst('Bad state: ', '');
            try {
              final restored = await native.send('request', op: 2);
              if (!_disposed) await _apply(restored);
            } catch (_) {
              /* preserve the last visible frame */
            }
            error = message;
            status = 'Could not finish that action';
            drawing = false;
            _timer?.cancel();
            _points.clear();
            _notify();
            complete.complete(null);
          }
        })
        .whenComplete(() {
          _pendingActions--;
          _notify();
        });
    return complete.future;
  }

  Future<void> _apply(Map<String, dynamic> result) async {
    state = Map<String, dynamic>.from(result['state'] as Map);
    final pixels = (result['rgba'] as TransferableTypedData)
        .materialize()
        .asUint8List();
    final decoded = Completer<ui.Image>();
    ui.decodeImageFromPixels(
      pixels,
      width,
      height,
      ui.PixelFormat.rgba8888,
      decoded.complete,
    );
    final next = await decoded.future;
    if (_disposed) {
      next.dispose();
      return;
    }
    final previous = image;
    image = next;
    // Keep the previous image alive through the current paint frame.
    Future<void>.delayed(
      const Duration(milliseconds: 100),
      () => previous?.dispose(),
    );
  }

  Future<void> command(
    int operation, {
    int a = 0,
    int b = 0,
    Uint8List? input,
  }) async {
    final result = await _enqueue(
      () => native.send('request', op: operation, a: a, b: b, input: input),
    );
    if (result != null) status = 'Ready to draw';
    _notify();
  }

  Uint8List _packet(List<Offset> points) {
    final bytes = ByteData(points.length * 8);
    for (var i = 0; i < points.length; i++) {
      bytes.setInt32(i * 8, points[i].dx.toInt(), Endian.little);
      bytes.setInt32(i * 8 + 4, points[i].dy.toInt(), Endian.little);
    }
    return bytes.buffer.asUint8List();
  }

  int _rgba = 0;
  void begin(Offset point) {
    if (!ready || busy || drawing || tool == 'Pan') return;
    drawing = true;
    _points.clear();
    final argb = tool == 'Erase' ? 0 : color;
    _rgba =
        ((argb >> 16) & 255) |
        (((argb >> 8) & 255) << 8) |
        ((argb & 255) << 16) |
        (((argb >> 24) & 255) << 24);
    final strokeColor = _rgba;
    final size = brush;
    unawaited(
      _enqueue(
        () => native.send(
          'stroke',
          phase: 0,
          input: _packet([point]),
          color: strokeColor,
          size: size,
        ),
      ),
    );
    _timer = Timer.periodic(const Duration(milliseconds: 16), (_) => _flush());
    _notify();
  }

  void move(Offset point) {
    if (!drawing) return;
    if (_points.length >= 256) _points.removeLast();
    if (_points.isEmpty || _points.last != point) _points.add(point);
  }

  void _flush() {
    if (_flushing || _points.isEmpty || !drawing) return;
    final points = List<Offset>.of(_points);
    _points.clear();
    _flushing = true;
    unawaited(
      _enqueue(
        () => native.send(
          'stroke',
          phase: 1,
          input: _packet(points),
          color: _rgba,
          size: brush,
        ),
      ).whenComplete(() => _flushing = false),
    );
  }

  Future<void> end() async {
    if (!drawing) return;
    drawing = false;
    _timer?.cancel();
    final points = List<Offset>.of(_points);
    _points.clear();
    final result = await _enqueue(
      () => native.send(
        'stroke',
        phase: 2,
        input: _packet(points),
        color: _rgba,
        size: brush,
      ),
    );
    if (result != null) status = 'Stroke complete';
    _notify();
  }

  Future<void> cancel() async {
    if (!drawing) return;
    drawing = false;
    _timer?.cancel();
    _points.clear();
    await _enqueue(() => native.send('stroke', phase: 3));
    status = 'Ready to draw';
    _notify();
  }

  void setTool(String value) {
    if (!drawing && !busy) {
      tool = value;
      _notify();
    }
  }

  void setColor(int value) {
    if (!drawing && !busy) {
      color = value;
      tool = 'Draw';
      _notify();
    }
  }

  void setBrush(int value) {
    if (!drawing && !busy) {
      brush = value;
      _notify();
    }
  }

  Future<void> newDocument({bool demo = false}) async {
    if (busy || drawing) return;
    busy = true;
    _notify();
    try {
      final result = await _enqueue(
        () => native.send('request', op: demo ? 1 : 0, a: 64, b: 64),
      );
      if (result != null) {
        savedKey = null;
        title = demo ? 'Moonlit garden' : 'Untitled';
        documentGeneration++;
        status = 'New canvas ready';
      }
    } finally {
      busy = false;
      _notify();
    }
  }

  Future<void> save({bool copy = false}) async {
    if (busy || drawing || !ready) return;
    busy = true;
    status = 'Saving…';
    _notify();
    try {
      final result = await _enqueue(() => native.send('request', op: 5));
      if (result == null) return;
      final bytes = (result['project'] as TransferableTypedData)
          .materialize()
          .asUint8List();
      final revision = (state['revision'] as num).toInt();
      if (copy) {
        final saved = await storage.invokeMethod<bool>('saveCopy', {
          'bytes': bytes,
        });
        if (saved != true) {
          status = 'Save cancelled';
          return;
        }
      } else {
        savedKey = await storage.invokeMethod<String>('save', {
          'bytes': bytes,
          'key': savedKey,
        });
      }
      final ack = await _enqueue(
        () => native.send('request', op: 12, a: revision),
      );
      if (ack != null) status = copy ? 'Copy saved' : 'Saved on this tablet';
    } catch (e) {
      error = 'Could not save: $e';
      status = 'Your artwork is still open';
    } finally {
      busy = false;
      _notify();
    }
  }

  Future<void> open({bool picker = false}) async {
    if (busy || drawing) return;
    busy = true;
    status = 'Opening…';
    _notify();
    try {
      final loaded = await storage.invokeMapMethod<String, dynamic>(
        picker ? 'openDocument' : 'reopen',
        {'key': savedKey},
      );
      if (loaded == null) {
        status = 'Open cancelled';
        return;
      }
      final bytes = loaded['bytes'] as Uint8List;
      final result = await _enqueue(
        () => native.send('request', op: 6, input: bytes),
      );
      if (result != null) {
        savedKey = loaded['key'] as String?;
        title = picker ? 'Imported sprite' : 'Saved sprite';
        documentGeneration++;
        status = 'Project reopened';
      }
    } catch (e) {
      error = 'Could not open: $e';
      status = 'Your current canvas is unchanged';
    } finally {
      busy = false;
      _notify();
    }
  }

  Future<void> settle() => _queue;
  @override
  void dispose() {
    _disposed = true;
    _timer?.cancel();
    image?.dispose();
    unawaited(native.close());
    super.dispose();
  }
}
