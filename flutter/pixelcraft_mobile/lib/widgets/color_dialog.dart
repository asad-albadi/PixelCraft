import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

Future<(String, int)?> editColor(BuildContext context, int initialARGB) =>
    showDialog<(String, int)>(
      context: context,
      builder: (_) => _ColorDialog(initialARGB: initialARGB),
    );

class _ColorDialog extends StatefulWidget {
  const _ColorDialog({required this.initialARGB});
  final int initialARGB;

  @override
  State<_ColorDialog> createState() => _ColorDialogState();
}

class _ColorDialogState extends State<_ColorDialog> {
  final _name = TextEditingController(text: 'Custom');
  late final TextEditingController _hex;
  late HSVColor _hsv;
  bool _validHex = true;
  Color get _color => _hsv.toColor();
  String get _rgb => (_color.toARGB32() & 0xffffff)
      .toRadixString(16)
      .padLeft(6, '0')
      .toUpperCase();

  @override
  void initState() {
    super.initState();
    _hsv = HSVColor.fromColor(Color(widget.initialARGB));
    _hex = TextEditingController(text: _rgb);
  }

  @override
  void dispose() {
    _name.dispose();
    _hex.dispose();
    super.dispose();
  }

  void _select(HSVColor value) => setState(() {
    _hsv = value;
    _hex.text = _rgb;
    _validHex = true;
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final opacity = (_hsv.alpha * 100).round();
    return Dialog(
      insetPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 24),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 400),
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(20),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text('Add color', style: theme.textTheme.titleLarge),
                  ),
                  IconButton(
                    tooltip: 'Close color picker',
                    constraints: const BoxConstraints(
                      minWidth: 48,
                      minHeight: 48,
                    ),
                    onPressed: () => Navigator.pop(context),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              _PickerInput(
                label: 'Saturation and brightness',
                value:
                    '${(_hsv.saturation * 100).round()}% saturation, ${(_hsv.value * 100).round()}% brightness',
                onAdjust: (dx, dy) => _select(
                  _hsv
                      .withSaturation((_hsv.saturation + dx).clamp(0, 1))
                      .withValue((_hsv.value - dy).clamp(0, 1)),
                ),
                onPosition: (position) => _select(
                  _hsv.withSaturation(position.dx).withValue(1 - position.dy),
                ),
                child: SizedBox(
                  key: const ValueKey('color-sv'),
                  height: 160,
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      DecoratedBox(
                        decoration: BoxDecoration(
                          color: HSVColor.fromAHSV(1, _hsv.hue, 1, 1).toColor(),
                        ),
                      ),
                      const DecoratedBox(
                        decoration: BoxDecoration(
                          gradient: LinearGradient(
                            colors: [Colors.white, Color(0x00ffffff)],
                          ),
                        ),
                      ),
                      const DecoratedBox(
                        decoration: BoxDecoration(
                          gradient: LinearGradient(
                            begin: Alignment.topCenter,
                            end: Alignment.bottomCenter,
                            colors: [Colors.transparent, Colors.black],
                          ),
                        ),
                      ),
                      _Marker(x: _hsv.saturation, y: 1 - _hsv.value),
                    ],
                  ),
                ),
              ),
              const SizedBox(height: 12),
              Text('Hue', style: theme.textTheme.labelLarge),
              _strip(
                key: const ValueKey('color-hue'),
                label: 'Hue',
                value: '${_hsv.hue.round()} degrees',
                position: _hsv.hue / 360,
                colors: const [
                  Color(0xffff0000),
                  Color(0xffffff00),
                  Color(0xff00ff00),
                  Color(0xff00ffff),
                  Color(0xff0000ff),
                  Color(0xffff00ff),
                  Color(0xffff0000),
                ],
                onChanged: (n) =>
                    _select(_hsv.withHue((n * 360).clamp(0, 359.999))),
              ),
              Text('Opacity · $opacity%', style: theme.textTheme.labelLarge),
              _strip(
                key: const ValueKey('color-opacity'),
                label: 'Opacity',
                value: '$opacity%',
                position: _hsv.alpha,
                colors: [_color.withAlpha(0), _color.withAlpha(255)],
                onChanged: (n) => _select(_hsv.withAlpha(n)),
              ),
              const SizedBox(height: 4),
              Row(
                children: [
                  Expanded(child: _swatch('Before', Color(widget.initialARGB))),
                  const SizedBox(width: 12),
                  Expanded(child: _swatch('After', _color)),
                ],
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _name,
                decoration: const InputDecoration(
                  labelText: 'Name',
                  isDense: true,
                  border: OutlineInputBorder(),
                ),
                textCapitalization: TextCapitalization.words,
                textInputAction: TextInputAction.next,
                onChanged: (_) => setState(() {}),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: _hex,
                decoration: InputDecoration(
                  labelText: 'RGB hex',
                  prefixText: '#',
                  isDense: true,
                  border: const OutlineInputBorder(),
                  errorText: _validHex ? null : 'Use six digits: 0–9 or A–F',
                ),
                autocorrect: false,
                enableSuggestions: false,
                textCapitalization: TextCapitalization.characters,
                onChanged: (text) => setState(() {
                  _validHex = RegExp(r'^[0-9a-fA-F]{6}$').hasMatch(text);
                  if (_validHex) {
                    final next = HSVColor.fromColor(
                      Color(0xff000000 | int.parse(text, radix: 16)),
                    );
                    _hsv = next
                        .withAlpha(_hsv.alpha)
                        .withHue(next.saturation == 0 ? _hsv.hue : next.hue);
                  }
                }),
              ),
              const SizedBox(height: 16),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: 8,
                runSpacing: 8,
                children: [
                  TextButton(
                    onPressed: () => Navigator.pop(context),
                    child: const Text('Cancel'),
                  ),
                  FilledButton(
                    onPressed: !_validHex || _name.text.trim().isEmpty
                        ? null
                        : () => Navigator.pop(context, (
                            _name.text.trim(),
                            _color.toARGB32(),
                          )),
                    child: const Text('Add'),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _swatch(String label, Color color) => Semantics(
    label: '$label color',
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(label, style: Theme.of(context).textTheme.labelMedium),
        const SizedBox(height: 4),
        ClipRRect(
          borderRadius: BorderRadius.circular(8),
          child: CustomPaint(
            painter: const _Checkerboard(),
            child: SizedBox(height: 32, child: ColoredBox(color: color)),
          ),
        ),
      ],
    ),
  );

  Widget _strip({
    required Key key,
    required String label,
    required String value,
    required double position,
    required List<Color> colors,
    required ValueChanged<double> onChanged,
  }) => _PickerInput(
    label: label,
    value: value,
    onAdjust: (dx, dy) => onChanged((position + dx - dy).clamp(0, 1)),
    onPosition: (p) => onChanged(p.dx),
    child: SizedBox(
      key: key,
      height: 48,
      child: Stack(
        fit: StackFit.expand,
        children: [
          Center(
            child: ClipRRect(
              borderRadius: BorderRadius.circular(8),
              child: CustomPaint(
                painter: const _Checkerboard(),
                child: Container(
                  height: 18,
                  decoration: BoxDecoration(
                    gradient: LinearGradient(colors: colors),
                  ),
                ),
              ),
            ),
          ),
          _Marker(x: position, y: 0.5),
        ],
      ),
    ),
  );
}

class _PickerInput extends StatefulWidget {
  const _PickerInput({
    required this.label,
    required this.value,
    required this.onAdjust,
    required this.onPosition,
    required this.child,
  });
  final String label;
  final String value;
  final void Function(double, double) onAdjust;
  final ValueChanged<Offset> onPosition;
  final Widget child;
  @override
  State<_PickerInput> createState() => _PickerInputState();
}

class _PickerInputState extends State<_PickerInput> {
  final _focus = FocusNode();
  bool _focused = false;
  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Semantics(
    label: '${widget.label}, ${widget.value}',
    hint: 'Use arrow keys to adjust',
    onIncrease: () => widget.onAdjust(0.01, 0),
    onDecrease: () => widget.onAdjust(-0.01, 0),
    child: Focus(
      focusNode: _focus,
      onFocusChange: (value) => setState(() => _focused = value),
      onKeyEvent: (_, event) {
        if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
          return KeyEventResult.ignored;
        }
        final key = event.logicalKey;
        if (key == LogicalKeyboardKey.arrowRight) {
          widget.onAdjust(0.01, 0);
        } else if (key == LogicalKeyboardKey.arrowLeft) {
          widget.onAdjust(-0.01, 0);
        } else if (key == LogicalKeyboardKey.arrowUp) {
          widget.onAdjust(0, -0.01);
        } else if (key == LogicalKeyboardKey.arrowDown) {
          widget.onAdjust(0, 0.01);
        } else {
          return KeyEventResult.ignored;
        }
        return KeyEventResult.handled;
      },
      child: LayoutBuilder(
        builder: (context, constraints) {
          void select(Offset point) {
            _focus.requestFocus();
            final box = context.findRenderObject()! as RenderBox;
            widget.onPosition(
              Offset(
                (point.dx / box.size.width).clamp(0, 1),
                (point.dy / box.size.height).clamp(0, 1),
              ),
            );
          }

          return GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTapDown: (details) => select(details.localPosition),
            onPanStart: (details) => select(details.localPosition),
            onPanUpdate: (details) => select(details.localPosition),
            child: DecoratedBox(
              position: DecorationPosition.foreground,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(8),
                border: Border.all(
                  color: _focused
                      ? Theme.of(context).colorScheme.primary
                      : Theme.of(context).colorScheme.outlineVariant,
                  width: _focused ? 2 : 1,
                ),
              ),
              child: ClipRRect(
                borderRadius: BorderRadius.circular(8),
                child: widget.child,
              ),
            ),
          );
        },
      ),
    ),
  );
}

class _Marker extends StatelessWidget {
  const _Marker({required this.x, required this.y});
  final double x;
  final double y;
  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (_, constraints) => Stack(
      children: [
        Positioned(
          left: (x * constraints.maxWidth - 9).clamp(
            1,
            constraints.maxWidth - 19,
          ),
          top: (y * constraints.maxHeight - 9).clamp(
            1,
            constraints.maxHeight - 19,
          ),
          child: Container(
            width: 18,
            height: 18,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              border: Border.all(color: Colors.white, width: 2),
              boxShadow: const [
                BoxShadow(
                  color: Colors.black54,
                  blurRadius: 2,
                  spreadRadius: 1,
                ),
              ],
            ),
          ),
        ),
      ],
    ),
  );
}

class _Checkerboard extends CustomPainter {
  const _Checkerboard();
  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRect(
      Offset.zero & size,
      Paint()..color = const Color(0xffeeeeee),
    );
    final paint = Paint()..color = const Color(0xffbdbdbd);
    for (double y = 0; y < size.height; y += 8) {
      for (double x = 0; x < size.width; x += 8) {
        if (((x / 8).floor() + (y / 8).floor()).isEven) {
          canvas.drawRect(
            Rect.fromLTWH(x, y, 8, 8).intersect(Offset.zero & size),
            paint,
          );
        }
      }
    }
  }

  @override
  bool shouldRepaint(_Checkerboard oldDelegate) => false;
}
