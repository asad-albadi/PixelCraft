import 'dart:async';
import 'dart:convert';
import 'dart:isolate';
import 'dart:math' as math;
import 'dart:ui' as ui;
import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter/services.dart';

import 'bridge/native_editor.dart';
import 'storage/recovery_store.dart';
import 'animation/playback_clock.dart';

class EditorController extends ChangeNotifier {
  final NativeEditor native;
  final RecoveryStore recovery;
  List<ui.Image> referenceImages = [];
  int selectedTile = 1;
  Uint8List? _recoverable;
  bool get recoveryPending => _recoverable != null || _recoveryReadFailed;
  Timer? _autosave;
  bool _initializing = false;
  bool importing = false;
  int _importGeneration = 0;
  void cancelImport() {
    if (!importing) return;
    _importGeneration++;
    storage.invokeMethod<dynamic>('cancelOpenDocument').ignore();
    importing = false;
    status = "Cancelling import…";
    _notify();
  }

  bool _suppressRecovery = false;
  bool _discardHandoff = false;
  bool _recoveryReadFailed = false;
  bool get canRestoreRecovery => _recoverable != null;
  bool get recoveryResolving => _fileBusy;
  String? _lastRecoveryIdentity;
  Future<void> _recoveryWork = Future.value();
  static const storage = MethodChannel('pixelcraft/storage');
  Map<String, dynamic> state = {};
  ui.Image? image;
  ui.Image? onionImage;
  bool get onionSkin => native.onion;
  final canvasUpdates = ValueNotifier<int>(0);
  bool get loadingFile => _initializing || _fileBusy;
  String status = 'Opening your canvas…';
  String? error;
  String title = 'Moonlit garden';
  String? savedKey;
  bool _fileBusy = false;
  bool playing = false;
  bool _startingPlayback = false;
  bool _foreground = true;
  Future<void>? _stoppingPlayback;
  double playbackSpeed = 1;
  int? previewFrame;
  int _playGeneration = 0;
  Timer? _playTimer;
  final _playClock = Stopwatch();
  int _pendingActions = 0;
  bool get busy =>
      playing ||
      _startingPlayback ||
      _initializing ||
      _fileBusy ||
      _pendingActions > 0 ||
      recoveryPending ||
      _discardHandoff;
  set busy(bool value) => _fileBusy = value;
  bool drawing = false;
  bool _disposed = false;
  bool _flushing = false;
  Timer? _timer;
  final List<Offset> _points = [];
  Future<void> _queue = Future.value();
  int color = 0xfff7e6b6;
  int brush = 1;
  int get brushShape => [
    'Round',
    'Square',
    'Diamond',
    'Horizontal',
    'Vertical',
  ].indexOf(state['brush_shape'] as String? ?? 'Round').clamp(0, 4);
  bool filled = false;
  bool stylusPressure = false;
  bool _pressureStroke = false;
  int _strokeSize = 1;
  void setStylusPressure(bool enabled) {
    if (!drawing && !busy) {
      stylusPressure = enabled;
      _notify();
    }
  }

  int pressureSize(double? pressure) => pressure == null || !pressure.isFinite
      ? brush
      : (brush * (.25 + .75 * pressure.clamp(0, 1))).round().clamp(1, 32);

  Offset? gestureStart, gestureEnd;
  String gestureTool = 'Draw';
  bool get _discreteTool => !['Draw', 'Erase', 'Pan'].contains(gestureTool);
  List<int> get rgba => [
    (color >> 16) & 255,
    (color >> 8) & 255,
    color & 255,
    (color >> 24) & 255,
  ];
  List<(String, int)> get namedColors {
    final palette = state['palette'] as List? ?? [];
    final names = state['palette_names'] as List? ?? [];
    return [
      for (var i = 0; i < names.length && i < palette.length; i++)
        if ((names[i] as String).isNotEmpty)
          (
            names[i] as String,
            ((palette[i][3] as int) << 24) |
                ((palette[i][0] as int) << 16) |
                ((palette[i][1] as int) << 8) |
                (palette[i][2] as int),
          ),
    ];
  }

  String tool = 'Draw';
  Rect? get selection {
    final bounds = state['selection'] as List?;
    if (bounds == null) return null;
    return Rect.fromLTRB(
      (bounds[0] as num).toDouble(),
      (bounds[1] as num).toDouble(),
      (bounds[2] as num).toDouble() + 1,
      (bounds[3] as num).toDouble() + 1,
    );
  }

  int documentGeneration = 0;
  bool zen = false;
  bool pixelGrid = false;
  void togglePixelGrid() {
    pixelGrid = !pixelGrid;
    _notify();
  }

  Future<void> exitZen() async {
    await cancel();
    if (!_disposed) setZen(false);
  }

  bool zenControlsLeft = false;
  bool timelineExpanded = true;
  final canvasTransform = TransformationController();
  Size canvasViewport = Size.zero, canvasDocumentSize = Size.zero;
  int canvasDocumentGeneration = -1;
  void setZen(bool enabled) {
    if (enabled && (drawing || busy || !ready)) return;
    if (zen == enabled) return;
    zen = enabled;
    _notify();
  }

  void setZenControlsLeft(bool left) {
    zenControlsLeft = left;
    _notify();
  }

  void setTimelineExpanded(bool expanded) {
    timelineExpanded = expanded;
    _notify();
  }

  void fitCanvas() {
    final size = canvasViewport;
    if (size.isEmpty) return;
    final scale =
        math.min(size.width / (width * 8), size.height / (height * 8)) * .76;
    canvasTransform.value = Matrix4.identity()
      ..translateByDouble(
        (size.width - width * 8 * scale) / 2,
        (size.height - height * 8 * scale) / 2,
        0,
        1,
      )
      ..scaleByDouble(scale, scale, 1, 1);
  }

  EditorController({NativeEditor? backend, RecoveryStore? recoveryStore})
    : native = backend ?? NativeEditor(),
      recovery = recoveryStore ?? RecoveryStore();
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
    _initializing = true;
    _notify();
    try {
      await native.start();
      await command(1);
      try {
        _recoverable = await recovery.read();
        if (recoveryPending) status = 'Unsaved artwork is available to recover';
      } catch (e) {
        _recoveryReadFailed = true;
        error = 'Recovery could not be read: $e';
      }
      _notify();
    } catch (e) {
      error = e.toString();
      status = 'Could not open the editor';
      _notify();
    } finally {
      _initializing = false;
      _notify();
    }
  }

  Future<Map<String, dynamic>?> _enqueue(
    Future<Map<String, dynamic>> Function() action, {
    bool Function()? accept,
    bool quiet = false,
  }) {
    final complete = Completer<Map<String, dynamic>?>();
    _pendingActions++;
    if (!quiet) _notify();
    _queue = _queue
        .then((_) async {
          if (_disposed) {
            complete.complete(null);
            return;
          }
          try {
            final result = await action();
            if (_disposed || (accept != null && !accept())) {
              complete.complete(null);
              return;
            }
            await _apply(result, accept: accept);
            error = null;
            if (!quiet) _notify();
            complete.complete(result);
          } catch (e) {
            if (_disposed || (accept != null && !accept())) {
              complete.complete(null);
              return;
            }
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
          if (!quiet || (_pendingActions == 0 && !drawing)) _notify();
        });
    return complete.future;
  }

  Future<void> _apply(
    Map<String, dynamic> result, {
    bool Function()? accept,
  }) async {
    if (accept != null && !accept()) return;
    final nextState = Map<String, dynamic>.from(result['state'] as Map);
    final pixels = (result['rgba'] as TransferableTypedData)
        .materialize()
        .asUint8List();
    final decoded = Completer<ui.Image>();
    ui.decodeImageFromPixels(
      pixels,
      nextState['width'] as int,
      nextState['height'] as int,
      ui.PixelFormat.rgba8888,
      decoded.complete,
    );
    final next = await decoded.future;
    ui.Image? onion;
    final extra = result['onion'] as TransferableTypedData?;
    if (extra != null) {
      final done = Completer<ui.Image>();
      ui.decodeImageFromPixels(
        extra.materialize().asUint8List(),
        nextState['width'] as int,
        nextState['height'] as int,
        ui.PixelFormat.rgba8888,
        done.complete,
      );
      onion = await done.future;
    }

    List<ui.Image>? nextReferences;
    if (result['referenceRgba'] is List) {
      nextReferences = [];
      final metadata = nextState['references'] as List? ?? [];
      final raw = result['referenceRgba'] as List;
      for (var i = 0; i < raw.length; i++) {
        final done = Completer<ui.Image>();
        ui.decodeImageFromPixels(
          (raw[i] as TransferableTypedData).materialize().asUint8List(),
          metadata[i]['width'] as int,
          metadata[i]['height'] as int,
          ui.PixelFormat.rgba8888,
          done.complete,
        );
        nextReferences.add(await done.future);
      }
    }
    if (_disposed || (accept != null && !accept())) {
      next.dispose();
      onion?.dispose();
      for (final image in nextReferences ?? <ui.Image>[]) {
        image.dispose();
      }
      return;
    }
    state = nextState;
    final oldReferences = nextReferences == null
        ? <ui.Image>[]
        : referenceImages;
    if (nextReferences != null) referenceImages = nextReferences;
    final previous = image;
    final oldOnion = onionImage;
    image = next;
    onionImage = onion;
    canvasUpdates.value++;
    // Keep the previous image alive through the current paint frame.
    Future<void>.delayed(const Duration(milliseconds: 100), () {
      previous?.dispose();
      for (final image in oldReferences) {
        image.dispose();
      }
      oldOnion?.dispose();
    });
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
    if (result != null) {
      status = 'Ready to draw';
      _scheduleRecovery();
    }
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
  void begin(Offset point, {bool eraser = false, double? pressure}) {
    if (_disposed || !ready || busy || drawing || (tool == 'Pan' && !eraser)) {
      return;
    }
    if (activeTileset != null && ['Draw', 'Erase'].contains(tool)) {
      unawaited(
        advanced(
          'paint-tile',
          extra: {
            'x': point.dx.toInt(),
            'y': point.dy.toInt(),
            'tile': eraser || tool == 'Erase' ? 0 : selectedTile,
          },
        ),
      );
      return;
    }
    _pressureStroke = stylusPressure && pressure != null;
    _strokeSize = _pressureStroke ? pressureSize(pressure) : brush;
    gestureTool = eraser ? 'Erase' : tool;
    drawing = true;
    status = 'Drawing';
    if (_discreteTool) {
      gestureStart = point;
      gestureEnd = point;
      canvasUpdates.value++;
      return;
    }
    _points.clear();
    final argb = gestureTool == 'Erase' ? 0 : color;
    _rgba =
        ((argb >> 16) & 255) |
        (((argb >> 8) & 255) << 8) |
        ((argb & 255) << 16) |
        (((argb >> 24) & 255) << 24);
    final strokeColor = _rgba;
    final size = _strokeSize;
    unawaited(
      _enqueue(
        () => native.send(
          'stroke',
          phase: 0,
          input: _packet([point]),
          color: strokeColor,
          size: size,
        ),
        quiet: true,
      ),
    );
    _timer = Timer.periodic(const Duration(milliseconds: 16), (_) => _flush());
    _notify();
  }

  void move(Offset point, {double? pressure}) {
    if (_disposed || !drawing) return;
    if (_pressureStroke && pressure != null) {
      _strokeSize = pressureSize(pressure);
    }
    if (_discreteTool) {
      gestureEnd = point;
      canvasUpdates.value++;
      return;
    }
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
          size: _strokeSize,
        ),
        quiet: true,
      ).whenComplete(() => _flushing = false),
    );
  }

  Future<void> end() async {
    if (!drawing) return;
    if (_discreteTool) {
      final from = gestureStart!, to = gestureEnd!;
      drawing = false;
      gestureStart = null;
      gestureEnd = null;
      if (gestureTool == 'Select') {
        await selectionAction(
          'select',
          extra: {
            'bounds': [
              from.dx.toInt(),
              from.dy.toInt(),
              to.dx.toInt(),
              to.dy.toInt(),
            ],
          },
        );
      } else if (gestureTool == 'Eyedropper') {
        final result = await _enqueue(
          () => native.send(
            'request',
            op: 19,
            a: to.dx.toInt(),
            b: to.dy.toInt(),
          ),
        );
        if (result != null) {
          final c = result['value']['color'] as List;
          color = (c[3] << 24) | (c[0] << 16) | (c[1] << 8) | c[2];
          tool = 'Draw';
        }
      } else {
        await edits([
          if (gestureTool == 'Fill')
            {
              'op': 'fill',
              'x': from.dx.toInt(),
              'y': from.dy.toInt(),
              'color': rgba,
            }
          else
            {
              'op': 'shape',
              'kind': gestureTool.toLowerCase(),
              'x': from.dx.toInt(),
              'y': from.dy.toInt(),
              'x2': to.dx.toInt(),
              'y2': to.dy.toInt(),
              'color': rgba,
              'filled': filled,
            },
        ]);
      }
      _notify();
      return;
    }
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
        size: _strokeSize,
      ),
    );
    if (result != null) {
      status = 'Stroke complete';
      _scheduleRecovery();
    }
    _notify();
  }

  Future<void> cancel() async {
    if (!drawing) return;
    drawing = false;
    if (_discreteTool) {
      gestureStart = null;
      gestureEnd = null;
      _notify();
      return;
    }
    _timer?.cancel();
    _points.clear();
    await _enqueue(() => native.send('stroke', phase: 3));
    status = 'Ready to draw';
    _notify();
  }

  Future<void> edits(List<Map<String, dynamic>> commands) async {
    if (busy || drawing || !ready) return;
    final layer = state['layer'], frame = state['frame'];
    final result = await _enqueue(
      () => native.send(
        'request',
        op: 17,
        input: Uint8List.fromList(
          utf8.encode(
            jsonEncode({
              'revision': state['revision'],
              'commands': [
                for (final c in commands)
                  {'layer': layer, 'frame': frame, ...c},
              ],
            }),
          ),
        ),
      ),
    );
    if (result != null) {
      status = 'Edit complete';
      _scheduleRecovery();
    }
    _notify();
  }

  Future<void> setBrushShape(int value) async {
    if (busy || drawing) return;
    await command(18, a: value);
  }

  void setFilled(bool value) {
    filled = value;
    _notify();
  }

  Future<void> addNamedColor(String name, int value) async {
    await edits([
      {
        'op': 'palette-entry',
        'name': name,
        'color': [
          (value >> 16) & 255,
          (value >> 8) & 255,
          value & 255,
          (value >> 24) & 255,
        ],
      },
    ]);
    if (error == null) setColor(value);
  }

  Future<void> selectionAction(
    String action, {
    Map<String, dynamic> extra = const {},
  }) async {
    if (busy || drawing || !ready) return;
    final dimensions = (width, height);
    final result = await _enqueue(
      () => native.send(
        'request',
        op: 20,
        input: Uint8List.fromList(
          utf8.encode(
            jsonEncode({
              'revision': state['revision'],
              'action': action,
              ...extra,
            }),
          ),
        ),
      ),
    );
    if (result != null) {
      if (dimensions != (width, height)) documentGeneration++;
      status = 'Selection updated';
      _scheduleRecovery();
    }
    _notify();
  }

  Future<void> addLayer({bool group = false}) async {
    final index = (state['layers'] as List).length;
    await edits([
      {
        'op': group ? 'add-group' : 'add-layer',
        'name': group ? 'Group' : 'Layer ${index + 1}',
      },
    ]);
    if (error == null) await command(10, a: index);
  }

  Future<void> addFrame({bool duplicate = false}) async {
    final frame = (state['frame'] as int) + 1;
    await edits([
      {'op': 'add-frame', 'duplicate': duplicate},
    ]);
    if (error == null) await command(9, a: frame);
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
    if (_initializing ||
        _fileBusy ||
        _pendingActions > 0 ||
        recoveryPending ||
        drawing) {
      return;
    }
    _discardHandoff = false;
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
        _scheduleRecovery();
      }
    } finally {
      busy = false;
      _scheduleRecovery();
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
      if (ack != null) {
        status = copy ? 'Copy saved' : 'Saved on this tablet';
        await recovery.clear();
        _lastRecoveryIdentity = null;
      }
    } catch (e) {
      error = 'Could not save: $e';
      status = 'Your artwork is still open';
    } finally {
      busy = false;
      _notify();
    }
  }

  Future<void> open({bool picker = false}) async {
    if (_initializing ||
        _fileBusy ||
        _pendingActions > 0 ||
        recoveryPending ||
        drawing) {
      return;
    }
    _discardHandoff = false;
    busy = true;
    importing = true;
    final generation = ++_importGeneration;
    status = 'Opening…';
    _notify();
    try {
      final loaded = await storage.invokeMapMethod<String, dynamic>(
        picker ? 'openDocument' : 'reopen',
        {'key': savedKey},
      );
      if (loaded == null || generation != _importGeneration) {
        status = 'Open cancelled';
        return;
      }
      final bytes = loaded['bytes'] as Uint8List;
      final prepared = await _enqueue(
        () => native.send(
          'request',
          op: 28,
          a: importFormat(bytes),
          input: bytes,
        ),
        accept: () => generation == _importGeneration,
      );
      if (prepared == null || generation != _importGeneration) {
        final failure = error;
        await _enqueue(() => native.send('request', op: 30));
        error = failure ?? error;
        if (generation != _importGeneration) status = 'Open cancelled';
        return;
      }
      // Commit is the short atomic boundary; cancellation is offered while preparing.
      importing = false;
      _notify();
      final result = await _enqueue(
        () => native.send('request', op: 29, a: state['revision'] as int),
      );
      if (result != null) {
        savedKey = loaded['key'] as String?;
        final name = loaded['name'] as String?;
        title = picker
            ? (name == null
                  ? 'Imported sprite'
                  : name.substring(0, math.min(128, name.length)))
            : 'Saved sprite';
        documentGeneration++;
        status = 'Project reopened';
        _scheduleRecovery();
      }
    } catch (e) {
      error = 'Could not open: $e';
      status = 'Your current canvas is unchanged';
    } finally {
      importing = false;
      busy = false;
      _scheduleRecovery();
      _notify();
    }
  }

  int? get activeTileset {
    final layers = state['layers'] as List? ?? [];
    final layer = state['layer'] as int? ?? 0;
    return layer < layers.length ? layers[layer]['tileset'] as int? : null;
  }

  void selectTile(int tile) {
    selectedTile = tile;
    _notify();
  }

  Future<void> advanced(
    String action, {
    Map<String, dynamic> extra = const {},
  }) async {
    if (busy || drawing || !ready) return;
    final result = await _enqueue(
      () => native.send(
        'request',
        op: 25,
        input: Uint8List.fromList(
          utf8.encode(
            jsonEncode({
              'revision': state['revision'],
              'action': action,
              ...extra,
            }),
          ),
        ),
      ),
    );
    if (result != null) {
      status = 'Edit complete';
      _scheduleRecovery();
    }
  }

  Future<void> importReference() async {
    if (busy || drawing || !ready) return;
    final sourceGeneration = documentGeneration;
    await flushRecovery();
    if (_disposed ||
        busy ||
        drawing ||
        !ready ||
        sourceGeneration != documentGeneration) {
      return;
    }
    busy = true;
    importing = true;
    final generation = ++_importGeneration;
    _notify();
    try {
      final loaded = await storage.invokeMapMethod<String, dynamic>(
        'openDocument',
      );
      if (loaded == null || generation != _importGeneration) {
        status = 'Reference import cancelled';
        return;
      }
      final bytes = loaded['bytes'] as Uint8List;
      if (importFormat(bytes) != 1) {
        throw StateError('Choose a PNG image for an embedded reference');
      }
      importing = false;
      _notify();
      final result = await _enqueue(
        () => native.send(
          'request',
          op: 26,
          a: state['revision'] as int,
          input: bytes,
        ),
      );
      if (result != null) {
        status = 'Reference added';
        _scheduleRecovery();
      }
    } catch (e) {
      error = 'Could not add reference: $e';
    } finally {
      importing = false;
      busy = false;
      _scheduleRecovery();
      _notify();
    }
  }

  static int importFormat(Uint8List bytes) {
    bool starts(List<int> signature) =>
        bytes.length >= signature.length &&
        List.generate(
          signature.length,
          (i) => bytes[i],
        ).asMap().entries.every((e) => e.value == signature[e.key]);
    if (starts([137, 80, 78, 71, 13, 10, 26, 10])) return 1;
    if (starts([71, 73, 70, 56, 55, 97]) || starts([71, 73, 70, 56, 57, 97])) {
      return 2;
    }
    if (bytes.length >= 128 && bytes[4] == 0xe0 && bytes[5] == 0xa5) return 3;
    return 0;
  }

  String? exportFolder;
  String? _sheetImageName, _sheetFolder;
  int? _sheetRevision;
  bool get sheetMetadataReady =>
      _sheetImageName != null &&
      _sheetFolder == exportFolder &&
      _sheetRevision == state['revision'];
  void _forgetSheetPair() {
    _sheetImageName = null;
    _sheetRevision = null;
    _sheetFolder = null;
  }

  Future<void> loadExportFolder() async {
    try {
      exportFolder = await storage.invokeMethod<String>('savedExportFolder');
      _notify();
    } catch (_) {
      /* A missing grant is handled by the next export. */
    }
  }

  Future<void> chooseExportFolder() async {
    if (busy || drawing) return;
    busy = true;
    _notify();
    try {
      final label = await storage.invokeMethod<String>('chooseExportFolder');
      if (label != null) {
        exportFolder = label;
        _forgetSheetPair();
      }
    } catch (e) {
      error = 'Could not choose export folder: $e';
    } finally {
      busy = false;
      _notify();
    }
  }

  Future<void> clearExportFolder() async {
    if (busy || drawing) return;
    busy = true;
    _notify();
    try {
      await storage.invokeMethod<void>('clearExportFolder');
      exportFolder = null;
      _forgetSheetPair();
    } catch (e) {
      error = 'Could not forget export folder: $e';
    } finally {
      busy = false;
      _notify();
    }
  }

  /// Export never acknowledges a native save or changes the dirty baseline.
  Future<void> exportArtwork(int format, {bool share = false}) async {
    if (busy || drawing || !ready) return;
    final entries = {
      1: ('PixelCraft.png', 'image/png'),
      2: ('PixelCraft.gif', 'image/gif'),
      3: ('PixelCraft.aseprite', 'application/octet-stream'),
      4: ('PixelCraft-sheet.png', 'image/png'),
      5: ('PixelCraft-sheet.json', 'application/json'),
    };
    final entry = entries[format];
    if (entry == null) return;
    if (!share && format == 4 && exportFolder == null) {
      await chooseExportFolder();
      if (exportFolder == null) return;
    }
    if (!share && format == 5 && !sheetMetadataReady) {
      error = 'Save the sprite sheet PNG first, then save its matching JSON to the same folder.';
      _notify();
      return;
    }
    final sourceGeneration = documentGeneration;
    await flushRecovery();
    if (_disposed ||
        busy ||
        drawing ||
        !ready ||
        sourceGeneration != documentGeneration) {
      return;
    }
    if (!share && format == 5 && !sheetMetadataReady) {
      error = 'Save the sprite sheet PNG again before exporting matching JSON.';
      _notify();
      return;
    }
    busy = true;
    status = share ? 'Preparing share…' : 'Exporting…';
    _notify();
    try {
      final result = await _enqueue(
        () => native.send('request', op: 24, a: format),
      );
      if (result == null) return;
      var bytes = (result['project'] as TransferableTypedData)
          .materialize()
          .asUint8List();
      var name = entry.$1;
      if (format == 5 && !share) {
        final metadata = jsonDecode(utf8.decode(bytes)) as Map<String, dynamic>;
        (metadata['meta'] as Map)['image'] = _sheetImageName;
        bytes = Uint8List.fromList(utf8.encode(jsonEncode(metadata)));
        name =
            '${_sheetImageName!.substring(0, _sheetImageName!.lastIndexOf('.'))}.json';
      }
      final done = await storage.invokeMethod<dynamic>(
        share
            ? 'shareDocument'
            : exportFolder == null
            ? 'exportDocument'
            : 'exportToFolder',
        {
          'bytes': bytes,
          'name': name,
          'mime': entry.$2,
          if (format == 4 && !share) 'returnName': true,
        },
      );
      if (format == 4 && !share && done is Map) {
        final actual = done['name'] as String?;
        if (actual == null ||
            !RegExp(r'^[A-Za-z0-9][A-Za-z0-9 _().-]{0,119}$')
                .hasMatch(actual) ||
            actual.contains('..') ||
            !actual.toLowerCase().endsWith('.png')) {
          throw StateError(
            'PNG saved, but its provider filename cannot be paired safely. Choose a short PNG basename.',
          );
        }
        _sheetImageName = actual;
        _sheetRevision = state['revision'] as int;
        _sheetFolder = exportFolder;
      }
      status = done == true || done is Map
          ? (share ? 'Share chooser opened' : 'Export saved')
          : 'Export cancelled';
    } catch (e) {
      error = 'Could not export: $e';
      status = 'Your artwork is still open';
    } finally {
      busy = false;
      _scheduleRecovery();
      _notify();
    }
  }

  void _scheduleRecovery() {
    _autosave?.cancel();
    if (!_disposed && !recoveryPending && !_initializing) {
      _suppressRecovery = false;
      _autosave = Timer(
        const Duration(milliseconds: 800),
        () => unawaited(flushRecovery()),
      );
    }
  }

  Future<void> flushRecovery() {
    _autosave?.cancel();
    _recoveryWork = _recoveryWork.then((_) async {
      if (_disposed ||
          _initializing ||
          _suppressRecovery ||
          recoveryPending ||
          !ready) {
        return;
      }
      if (playing) {
        _scheduleRecovery();
        return;
      }
      await settle();
      if (_disposed || recoveryPending || drawing || _fileBusy) {
        _scheduleRecovery();
        return;
      }
      final generation = documentGeneration;
      final revision = (state['revision'] as num).toInt();
      final identity =
          '$generation:$revision:$dirty:${state['frame']}:${state['layer']}:${state['brush_shape']}';
      if (identity == _lastRecoveryIdentity) return;
      try {
        if (dirty) {
          final result = await _enqueue(() => native.send('request', op: 15));
          if (result == null ||
              generation != documentGeneration ||
              _disposed ||
              _suppressRecovery) {
            return;
          }
          await recovery.write(
            (result['recovery'] as TransferableTypedData)
                .materialize()
                .asUint8List(),
          );
        } else {
          await recovery.clear();
        }
        _lastRecoveryIdentity = identity;
      } catch (e) {
        error = 'Autosave failed. Use Save to protect your artwork: $e';
        _notify();
      }
    });
    return _recoveryWork;
  }

  Future<void> resolveRecovery({required bool restore}) async {
    if (!recoveryPending || _fileBusy) return;
    _fileBusy = true;
    _notify();
    try {
      if (!restore) {
        try {
          await recovery.clear();
          _recoverable = null;
          _recoveryReadFailed = false;
          status = 'Recovery discarded';
        } catch (e) {
          error = 'Could not discard recovery: $e';
        }
      } else {
        final result = await _enqueue(
          () => native.send('request', op: 16, input: _recoverable),
        );
        if (result != null) {
          _recoverable = null;
          savedKey = null;
          title = 'Recovered sprite';
          documentGeneration++;
          status = 'Recovered unsaved artwork — Save to keep a project copy';
        }
      }
    } finally {
      _fileBusy = false;
      _notify();
    }
  }

  Future<bool> discardRecovery() async {
    _suppressRecovery = true;
    _discardHandoff = true;
    _notify();
    _autosave?.cancel();
    try {
      await _recoveryWork;
      await recovery.clear();
      _lastRecoveryIdentity = null;
      return true;
    } catch (e) {
      _discardHandoff = false;
      _scheduleRecovery();
      error = 'Could not discard recovery: $e';
      _notify();
      return false;
    }
  }

  Future<void> setOnionSkin(bool enabled) async {
    if (busy || drawing) return;
    native.onion = enabled;
    await command(2);
  }

  Future<void> startPlayback({int? tag}) async {
    if (busy || drawing || !ready || !_foreground) return;
    final generation = ++_playGeneration;
    _startingPlayback = true;
    _notify();
    await flushRecovery();
    if (generation != _playGeneration || !_foreground || _disposed) {
      return;
    }
    final info = await _enqueue(
      () => native.send('request', op: 22, a: tag ?? 0, b: tag == null ? 0 : 1),
    );
    if (info == null ||
        _disposed ||
        generation != _playGeneration ||
        !_foreground) {
      if (generation == _playGeneration) {
        _startingPlayback = false;
        _notify();
      }
      return;
    }
    final data = info['value'] as Map;
    final schedule = PlaybackClock(
      List<int>.from(data['sequence'] as List),
      List<int>.from(data['durations'] as List),
      speed: playbackSpeed,
      repeat: data['repeat'] as int,
    );
    _startingPlayback = false;
    playing = true;
    _playClock
      ..reset()
      ..start();
    status = 'Playing animation';
    _notify();
    Future<void> tick() async {
      if (!playing || _disposed || generation != _playGeneration) return;
      final step = schedule.sample(_playClock.elapsed);
      if (step.finished) {
        await stopPlayback();
        return;
      }
      if (previewFrame != step.frame) {
        final result = await _enqueue(
          () => native.send('preview', a: step.frame),
          accept: () => playing && generation == _playGeneration,
        );
        if (result == null) {
          if (playing && generation == _playGeneration) await stopPlayback();
          return;
        }
        if (!playing || generation != _playGeneration) return;
        previewFrame = step.frame;
        _notify();
      }
      final now = schedule.sample(_playClock.elapsed);
      _playTimer = Timer(
        now.finished || now.frame != previewFrame
            ? const Duration(milliseconds: 1)
            : now.wait,
        () => unawaited(tick()),
      );
    }

    unawaited(tick());
  }

  Future<void> stopPlayback() {
    _playGeneration++;
    _startingPlayback = false;
    if (!playing) {
      _notify();
      return _stoppingPlayback ?? Future.value();
    }
    playing = false;
    _playTimer?.cancel();
    _playClock.stop();
    previewFrame = null;
    final stopped = command(2).whenComplete(() {
      _stoppingPlayback = null;
      _notify();
    });
    _stoppingPlayback = stopped;
    return stopped;
  }

  void setPlaybackSpeed(double speed) {
    if (!playing) {
      playbackSpeed = speed;
      _notify();
    }
  }

  void resume() {
    _foreground = true;
  }

  Future<void> pause() async {
    _foreground = false;
    await stopPlayback();
    await cancel();
    await flushRecovery();
  }

  Future<void> settle() => _queue;
  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _playGeneration++;
    _playTimer?.cancel();
    _playClock.stop();
    _timer?.cancel();
    _autosave?.cancel();
    image?.dispose();
    onionImage?.dispose();
    for (final image in referenceImages) {
      image.dispose();
    }
    canvasUpdates.dispose();
    canvasTransform.dispose();
    unawaited(native.close());
    super.dispose();
  }
}
