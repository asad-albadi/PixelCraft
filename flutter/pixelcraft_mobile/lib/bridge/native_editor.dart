import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:isolate';
import 'dart:typed_data';

/// Owns all FFI calls and borrowed memory on one serialized worker isolate.
class NativeEditor {
  Isolate? _isolate;
  final _messages = ReceivePort();
  final _errors = ReceivePort();
  final _exits = ReceivePort();
  final _ready = Completer<SendPort>();
  final Map<int, Completer<Map<String, dynamic>>> _pending = {};
  int _next = 0;
  bool _closed = false;
  bool onion = false;
  Future<void>? _closeFuture;
  bool _closing = false;
  bool _started = false;
  String? _fatal;
  NativeEditor() {
    _ready.future.ignore();
  }

  Future<void> start({
    String library = const String.fromEnvironment(
      'PIXELCRAFT_NATIVE_LIBRARY',
      defaultValue: 'libpixelcraft_ffi.so',
    ),
  }) async {
    if (_closed || _started) {
      throw StateError('Editor already started or closed');
    }
    _started = true;
    _messages.listen((message) {
      if (message is SendPort) {
        if (!_ready.isCompleted) _ready.complete(message);
        return;
      }
      final data = Map<String, dynamic>.from(message as Map);
      if (data.containsKey('startupError')) {
        _fail(data['startupError'].toString());
        return;
      }
      final completer = _pending.remove(data['id']);
      if (completer == null) return;
      if (data['error'] != null) {
        completer.completeError(StateError(data['error'].toString()));
      } else {
        completer.complete(data);
      }
    });
    _errors.listen((error) => _fail('Editor worker stopped: $error'));
    _exits.listen((_) {
      if (!_closed && !_closing) _fail('Editor worker closed unexpectedly');
    });
    _isolate = await Isolate.spawn(
      _worker,
      [_messages.sendPort, library],
      onError: _errors.sendPort,
      onExit: _exits.sendPort,
    );
    if (_closed || _closing) {
      _isolate?.kill();
      throw StateError('Editor is closed');
    }
    await _ready.future;
  }

  void _fail(String error) {
    _fatal = error;
    if (_started && !_ready.isCompleted) {
      _ready.completeError(StateError(error));
    }
    for (final request in _pending.values) {
      request.completeError(StateError(error));
    }
    _pending.clear();
  }

  Future<Map<String, dynamic>> send(
    String action, {
    int op = 2,
    int a = 0,
    int b = 0,
    Uint8List? input,
    int phase = 0,
    int color = 0,
    int size = 1,
  }) async {
    if (_closed) throw StateError('Editor is closed');
    if (_fatal != null) throw StateError(_fatal!);
    if (_pending.length >= 8) throw StateError('Editor is busy');
    final port = await _ready.future;
    if (_closed) throw StateError('Editor is closed');
    final id = ++_next;
    final result = Completer<Map<String, dynamic>>();
    _pending[id] = result;
    port.send({
      'id': id,
      'onion': onion,
      'action': action,
      'op': op,
      'a': a,
      'b': b,
      'phase': phase,
      'color': color,
      'size': size,
      'input': input == null ? null : TransferableTypedData.fromList([input]),
    });
    return result.future;
  }

  Future<void> close() => _closeFuture ??= _close();
  Future<void> _close() async {
    if (_closed || _closing) return;
    _closing = true;
    try {
      if (_ready.isCompleted && _fatal == null) {
        await send('close').timeout(const Duration(seconds: 2));
      }
    } catch (_) {
      /* isolate is already gone */
    }
    _closed = true;
    _isolate?.kill();
    _fail('Editor is closed');
    _messages.close();
    _errors.close();
    _exits.close();
  }
}

void _worker(List<Object> init) {
  final parent = init[0] as SendPort;
  final commands = ReceivePort();
  late _Bindings native;
  try {
    native = _Bindings(DynamicLibrary.open(init[1] as String));
  } catch (error) {
    parent.send({'startupError': error.toString()});
    commands.close();
    return;
  }
  var session = 0;
  var strokeSize = 1;
  parent.send(commands.sendPort);
  commands.listen((message) {
    final command = Map<String, dynamic>.from(message as Map);
    var input = 0;
    try {
      final transferred = command['input'] as TransferableTypedData?;
      if (transferred != null) {
        input = native.input(transferred.materialize().asUint8List());
      }
      final result = <String, dynamic>{'id': command['id']};
      if (command['action'] == 'close') {
        if (session != 0) native.close(session);
        session = 0;
        parent.send(result);
        commands.close();
        return;
      }
      if (command['action'] == 'stroke') {
        final phase = command['phase'] as int, size = command['size'] as int;
        if ((phase == 1 || phase == 2) && size != strokeSize) {
          native.read(native.request(session, 31, size, 0, 0));
        }
        if (phase != 3) strokeSize = size;
        final status = native.stroke(
          session,
          command['phase'] as int,
          input,
          command['color'] as int,
          command['size'] as int,
        );
        if (status != 0) {
          native.stroke(session, 3, 0, 0, 1);
          throw StateError(
            'Cannot draw there. Choose an unlocked image layer or a smaller project.',
          );
        }
      } else if (command['action'] != 'preview') {
        final response = native.read(
          native.request(
            session,
            command['op'] as int,
            command['a'] as int,
            command['b'] as int,
            input,
          ),
        );
        if (response.$1 == 1) {
          final state =
              jsonDecode(utf8.decode(response.$2)) as Map<String, dynamic>;
          session = (state['session'] as num).toInt();
        } else if (response.$1 == 6) {
          result['value'] = jsonDecode(utf8.decode(response.$2));
        } else if (response.$1 == 5) {
          result['recovery'] = TransferableTypedData.fromList([response.$2]);
        } else if (response.$1 == 3) {
          result['project'] = TransferableTypedData.fromList([response.$2]);
        }
      }
      final state = native.read(native.request(session, 2, 0, 0, 0));
      result['state'] = jsonDecode(utf8.decode(state.$2));
      if (command['action'] != 'preview' &&
          [0, 1, 3, 4, 6, 16, 23, 25, 26, 29].contains(command['op'])) {
        final refs = (result['state'] as Map)['references'] as List? ?? [];
        result['referenceRgba'] = [
          for (var i = 0; i < refs.length; i++)
            TransferableTypedData.fromList([
              native.read(native.request(session, 27, i, 0, 0)).$2,
            ]),
        ];
      }

      final frame = native.read(
        native.request(
          session,
          7,
          command['action'] == 'preview' ? command['a'] as int : 0,
          command['action'] == 'preview' ? 1 : 0,
          0,
        ),
      );
      result['rgba'] = TransferableTypedData.fromList([frame.$2]);
      if (command['onion'] == true && command['action'] != 'preview') {
        final image = native.read(native.request(session, 21, 1, 1, 0));
        result['onion'] = TransferableTypedData.fromList([image.$2]);
      }
      parent.send(result);
    } catch (error) {
      if (command['action'] == 'stroke' && session != 0) {
        try {
          native.stroke(session, 3, 0, 0, 1);
        } catch (_) {}
      }
      parent.send({'id': command['id'], 'error': error.toString()});
    } finally {
      if (input != 0) native.free(input);
    }
  });
}

class _Bindings {
  final int Function() abi;
  final int Function(int) alloc;
  final Pointer<Uint8> Function(int) data;
  final int Function(int) length;
  final int Function(int) kind;
  final int Function(int) free;
  final int Function(int) close;
  final int Function(int, int, int, int, int) request;
  final int Function(int, int, int, int, int) stroke;
  _Bindings(DynamicLibrary lib)
    : abi = lib.lookupFunction<Uint32 Function(), int Function()>(
        'pc_abi_version',
      ),
      alloc = lib.lookupFunction<Uint64 Function(Size), int Function(int)>(
        'pc_buffer_alloc',
      ),
      data = lib
          .lookupFunction<
            Pointer<Uint8> Function(Uint64),
            Pointer<Uint8> Function(int)
          >('pc_buffer_data'),
      length = lib.lookupFunction<Size Function(Uint64), int Function(int)>(
        'pc_buffer_len',
      ),
      kind = lib.lookupFunction<Uint32 Function(Uint64), int Function(int)>(
        'pc_buffer_kind',
      ),
      free = lib.lookupFunction<Uint32 Function(Uint64), int Function(int)>(
        'pc_buffer_free',
      ),
      close = lib.lookupFunction<Uint32 Function(Uint64), int Function(int)>(
        'pc_session_close',
      ),
      request = lib
          .lookupFunction<
            Uint64 Function(Uint64, Uint32, Uint32, Uint32, Uint64),
            int Function(int, int, int, int, int)
          >('pc_request'),
      stroke = lib
          .lookupFunction<
            Int32 Function(Uint64, Uint32, Uint64, Uint32, Uint32),
            int Function(int, int, int, int, int)
          >('pc_stroke') {
    if (abi() != 1) throw StateError('Editor library version is incompatible');
  }
  int input(Uint8List bytes) {
    final id = alloc(bytes.length);
    if (id == 0) throw StateError('Input exceeds the editor memory budget');
    data(id).asTypedList(bytes.length).setAll(0, bytes);
    return id;
  }

  (int, Uint8List) read(int id) {
    if (id == 0) throw StateError('Editor memory budget reached');
    try {
      final type = kind(id);
      final size = length(id);
      if (size > 8 * 1024 * 1024 || data(id) == nullptr) {
        throw StateError('Invalid editor buffer');
      }
      final bytes = Uint8List.fromList(data(id).asTypedList(size));
      if (type == 0) throw StateError(utf8.decode(bytes));
      return (type, bytes);
    } finally {
      free(id);
    }
  }
}
