import 'package:flutter/material.dart';

import '../editor_controller.dart';
import 'document_tools.dart';

void showAdvancedTools(BuildContext context, EditorController editor) =>
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      constraints: const BoxConstraints(maxWidth: 640),
      builder: (_) => SizedBox(
        height: MediaQuery.sizeOf(context).height * .78,
        child: AdvancedTools(editor: editor),
      ),
    );

class AdvancedTools extends StatelessWidget {
  final EditorController editor;
  const AdvancedTools({super.key, required this.editor});
  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: editor,
    builder: (context, _) {
      final e = editor, enabled = e.ready && !e.busy && !e.drawing;
      final sets = e.state['tilesets'] as List? ?? [],
          refs = e.state['references'] as List? ?? [];
      return SafeArea(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
          children: [
            Text(
              'Document tools',
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 16),
            Text(
              'Color mode: ${e.state['mode'] ?? 'Rgb'}',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const Text(
              'Conversion uses the shared editor engine and can be undone.',
            ),
            Wrap(
              spacing: 8,
              children: [
                for (final item in [
                  ('RGB', 'convert-rgb'),
                  ('Grayscale', 'convert-grayscale'),
                  ('Indexed', 'convert-indexed'),
                ])
                  OutlinedButton(
                    onPressed: enabled
                        ? () => e.edits([
                            {'op': item.$2},
                          ])
                        : null,
                    child: Text(item.$1),
                  ),
              ],
            ),
            const Divider(height: 32),
            Text('Tilemaps', style: Theme.of(context).textTheme.titleMedium),
            const Text(
              'Create tiles from this frame. Original layers are retained, hidden underneath the new tile layer. Use Pencil to tap cells and stamp the chosen tile; Eraser stamps an empty cell.',
            ),
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.grid_view_outlined),
              title: const Text('Create tileset from frame…'),
              enabled: enabled,
              onTap: enabled
                  ? () async {
                      final w = await askText(
                        context,
                        'Tile width (1–64 pixels)',
                        '8',
                        number: true,
                      );
                      if (w == null || !context.mounted) return;
                      final h = await askText(
                        context,
                        'Tile height (1–64 pixels)',
                        '8',
                        number: true,
                      );
                      final width = int.tryParse(w),
                          height = int.tryParse(h ?? '');
                      if (width == null ||
                          height == null ||
                          width < 1 ||
                          width > 64 ||
                          height < 1 ||
                          height > 64) {
                        return;
                      }
                      await e.advanced(
                        'create-tileset',
                        extra: {
                          'tile_width': width,
                          'tile_height': height,
                          'hide_source': true,
                        },
                      );
                    }
                  : null,
            ),
            for (final set in sets)
              ExpansionTile(
                title: Text(
                  '${set['name']} · ${set['width']} × ${set['height']}',
                ),
                subtitle: Text('${set['count']} tiles'),
                initiallyExpanded: e.activeTileset == set['index'],
                children: [
                  Padding(
                    padding: const EdgeInsets.all(8),
                    child: Wrap(
                      spacing: 4,
                      runSpacing: 4,
                      children: [
                        for (var i = 0; i <= (set['count'] as int); i++)
                          ChoiceChip(
                            label: Text(i == 0 ? 'Empty' : '$i'),
                            selected:
                                e.activeTileset == set['index'] &&
                                e.selectedTile == i,
                            onSelected:
                                enabled && e.activeTileset == set['index']
                                ? (_) => e.selectTile(i)
                                : null,
                          ),
                      ],
                    ),
                  ),
                ],
              ),
            const Divider(height: 32),
            Text(
              'Embedded references',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const Text(
              'References guide drawing and stay in .pixel projects; normal artwork exports leave them out. PNG imports use the mobile canvas limits.',
            ),
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.add_photo_alternate_outlined),
              title: const Text('Add PNG reference…'),
              enabled: enabled && refs.length < 4,
              onTap: enabled && refs.length < 4
                  ? () => e.importReference()
                  : null,
            ),
            for (final ref in refs.asMap().entries)
              ListTile(
                contentPadding: EdgeInsets.zero,
                title: Text('${ref.value['name']}'),
                subtitle: Text(
                  '${ref.value['width']} × ${ref.value['height']} · ${ref.value['visible'] == true ? 'Visible' : 'Hidden'}',
                ),
                leading: ref.key < e.referenceImages.length
                    ? SizedBox(
                        width: 48,
                        height: 48,
                        child: RawImage(
                          image: e.referenceImages[ref.key],
                          filterQuality: FilterQuality.none,
                          fit: BoxFit.contain,
                        ),
                      )
                    : const Icon(Icons.image_outlined),
                trailing: PopupMenuButton<String>(
                  tooltip: 'Reference options ${ref.key + 1}',
                  enabled: enabled && ref.value['locked'] != true,
                  itemBuilder: (_) => [
                    const PopupMenuItem(
                      value: 'properties',
                      child: Text('Position & opacity…'),
                    ),
                    PopupMenuItem(
                      value: 'toggle',
                      child: Text(
                        ref.value['visible'] == true
                            ? 'Hide reference'
                            : 'Show reference',
                      ),
                    ),
                    const PopupMenuItem(
                      value: 'layer',
                      child: Text('Convert to layer'),
                    ),
                    const PopupMenuItem(
                      value: 'remove',
                      child: Text('Remove reference'),
                    ),
                  ],
                  onSelected: (action) async {
                    if (action == 'properties') {
                      await showDialog<void>(
                        context: context,
                        builder: (_) => _ReferenceDialog(
                          editor: e,
                          index: ref.key,
                          reference: Map<String, dynamic>.from(ref.value),
                        ),
                      );
                      return;
                    }
                    if (action == 'toggle') {
                      await e.advanced(
                        'reference-set',
                        extra: {
                          'index': ref.key,
                          'x': ref.value['x'],
                          'y': ref.value['y'],
                          'scale': ref.value['scale'],
                          'opacity': ref.value['opacity'],
                          'visible': ref.value['visible'] != true,
                          'scale_y': ref.value['scale_y'],
                        },
                      );
                    } else {
                      await e.advanced(
                        action == 'layer'
                            ? 'reference-to-layer'
                            : 'reference-remove',
                        extra: {'index': ref.key},
                      );
                    }
                  },
                ),
              ),
            if (e.error != null)
              Text(
                e.error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
          ],
        ),
      );
    },
  );
}

class _ReferenceDialog extends StatefulWidget {
  final EditorController editor;
  final int index;
  final Map<String, dynamic> reference;
  const _ReferenceDialog({
    required this.editor,
    required this.index,
    required this.reference,
  });
  @override
  State<_ReferenceDialog> createState() => _ReferenceDialogState();
}

class _ReferenceDialogState extends State<_ReferenceDialog> {
  late final x = TextEditingController(text: '${widget.reference['x']}'),
      y = TextEditingController(text: '${widget.reference['y']}'),
      scale = TextEditingController(text: '${widget.reference['scale']}');
  late double opacity = (widget.reference['opacity'] as num).toDouble();
  String? error;
  @override
  void dispose() {
    x.dispose();
    y.dispose();
    scale.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: widget.editor,
    builder: (context, _) => AlertDialog(
      title: const Text('Reference position'),
      content: SizedBox(
        width: 340,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final item in [
                ('X position', x),
                ('Y position', y),
                ('Uniform scale', scale),
              ])
                TextField(
                  controller: item.$2,
                  decoration: InputDecoration(labelText: item.$1),
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                    signed: true,
                  ),
                ),
              Text('Opacity ${(opacity / 255 * 100).round()}%'),
              Slider(
                value: opacity,
                min: 0,
                max: 255,
                onChanged: (v) => setState(() => opacity = v),
              ),
              if (error != null) Text(error!),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: widget.editor.busy
              ? null
              : () async {
                  final xx = double.tryParse(x.text),
                      yy = double.tryParse(y.text),
                      ss = double.tryParse(scale.text);
                  if (xx == null ||
                      yy == null ||
                      ss == null ||
                      !xx.isFinite ||
                      !yy.isFinite ||
                      !ss.isFinite ||
                      xx.abs() > 4096 ||
                      yy.abs() > 4096 ||
                      ss < .01 ||
                      ss > 16) {
                    setState(
                      () => error = 'Use finite positions within ±4096 and scale 0.01–16.',
                    );
                    return;
                  }
                  await widget.editor.advanced(
                    'reference-set',
                    extra: {
                      'index': widget.index,
                      'x': xx,
                      'y': yy,
                      'scale': ss,
                      'scale_y': null,
                      'opacity': opacity.round(),
                      'visible': widget.reference['visible'],
                    },
                  );
                  if (context.mounted && widget.editor.error == null) {
                    Navigator.pop(context);
                  }
                },
          child: const Text('Apply'),
        ),
      ],
    ),
  );
}
