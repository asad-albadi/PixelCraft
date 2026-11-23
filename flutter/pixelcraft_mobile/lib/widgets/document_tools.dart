import 'package:flutter/material.dart';

import '../editor_controller.dart';

Future<String?> askText(
  BuildContext context,
  String title,
  String initial, {
  bool number = false,
}) => showDialog<String>(
  context: context,
  builder: (_) => _TextPrompt(title, initial, number),
);

class _TextPrompt extends StatefulWidget {
  final String title, initial;
  final bool number;
  const _TextPrompt(this.title, this.initial, this.number);
  @override
  State<_TextPrompt> createState() => _TextPromptState();
}

class _TextPromptState extends State<_TextPrompt> {
  late final value = TextEditingController(text: widget.initial);
  @override
  void dispose() {
    value.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: Text(widget.title),
    scrollable: true,
    content: TextField(
      controller: value,
      autofocus: true,
      maxLength: 128,
      keyboardType: widget.number ? TextInputType.number : TextInputType.text,
      onSubmitted: (_) => Navigator.pop(context, value.text),
    ),
    actions: [
      TextButton(
        onPressed: () => Navigator.pop(context),
        child: const Text('Cancel'),
      ),
      FilledButton(
        onPressed: () => Navigator.pop(context, value.text),
        child: const Text('Apply'),
      ),
    ],
  );
}

void showDocumentTools(
  BuildContext context,
  EditorController editor, {
  int tab = 0,
}) => showModalBottomSheet<void>(
  context: context,
  isScrollControlled: true,
  showDragHandle: true,
  constraints: const BoxConstraints(maxWidth: 640),
  builder: (_) => SizedBox(
    height: MediaQuery.sizeOf(context).height * .72,
    child: DocumentPanel(editor: editor, section: tab),
  ),
);

/// Context-specific controls. Layer rows stay visible rather than hidden in a selector.
class DocumentPanel extends StatelessWidget {
  final EditorController editor;
  final int section;
  const DocumentPanel({super.key, required this.editor, this.section = 0});
  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: editor,
    builder: (context, _) {
      final e = editor;
      final enabled = e.ready && !e.busy && !e.drawing;
      final layer = (e.state['layer'] as int?) ?? 0,
          frame = (e.state['frame'] as int?) ?? 0;
      final layers = e.state['layers'] as List? ?? [],
          frames = (e.state['frames'] as int?) ?? 1;
      final current = layers.isEmpty
          ? <String, dynamic>{}
          : layers[layer] as Map;
      final tags = e.state['tags'] as List? ?? [];
      Widget action(String label, IconData icon, VoidCallback? call) =>
          ListTile(
            leading: Icon(icon),
            title: Text(label),
            enabled: enabled && call != null,
            onTap: enabled ? call : null,
          );
      return SafeArea(
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 8, 8),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      [
                        'Layers',
                        'Frame ${frame + 1} of $frames',
                        'Animation tags',
                        'Selection',
                      ][section],
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                  ),
                  IconButton(
                    tooltip: 'Close panel',
                    onPressed: () => Navigator.pop(context),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
            ),
            const Divider(height: 1),
            if (e.error != null)
              Padding(
                padding: const EdgeInsets.all(12),
                child: Text(
                  e.error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.only(bottom: 16),
                children: [
                  if (section == 0) ...[
                    Padding(
                      padding: const EdgeInsets.all(12),
                      child: Wrap(
                        spacing: 8,
                        children: [
                          FilledButton.tonalIcon(
                            onPressed: enabled && layers.length < 32
                                ? () => e.addLayer()
                                : null,
                            icon: const Icon(Icons.add),
                            label: const Text('Add layer'),
                          ),
                          OutlinedButton.icon(
                            onPressed: enabled && layers.length < 32
                                ? () => e.addLayer(group: true)
                                : null,
                            icon: const Icon(Icons.create_new_folder_outlined),
                            label: const Text('Add group'),
                          ),
                        ],
                      ),
                    ),
                    for (final item in layers.asMap().entries.toList().reversed)
                      LayerRow(editor: e, index: item.key),
                  ],
                  if (section == 1) ...[
                    SwitchListTile(
                      title: const Text('Onion skin'),
                      subtitle: const Text('Adjacent frames on canvas'),
                      value: e.onionSkin,
                      onChanged: enabled ? (v) => e.setOnionSkin(v) : null,
                    ),
                    action(
                      'Add blank frame',
                      Icons.add,
                      frames < 64 ? () => e.addFrame() : null,
                    ),
                    action(
                      'Duplicate frame',
                      Icons.copy_outlined,
                      frames < 64 ? () => e.addFrame(duplicate: true) : null,
                    ),
                    action('Frame duration', Icons.timer_outlined, () async {
                      final text = await askText(
                        context,
                        'Duration in milliseconds',
                        '${e.state['durations'][frame]}',
                        number: true,
                      );
                      if (text == null) return;
                      final value = int.tryParse(text);
                      if (value == null || value < 1 || value > 65535) {
                        if (context.mounted) {
                          ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(
                              content: Text('Use 1–65535 milliseconds.'),
                            ),
                          );
                        }
                        return;
                      }
                      await e.edits([
                        {'op': 'set-frame', 'duration': value},
                      ]);
                    }),
                    action('Frame name', Icons.label_outline, () async {
                      final names = e.state['frame_names'] as List? ?? [];
                      final name = await askText(
                        context,
                        'Frame name',
                        frame < names.length ? '${names[frame]}' : '',
                      );
                      if (name != null) {
                        await e.edits([
                          {'op': 'set-frame', 'name': name},
                        ]);
                      }
                    }),
                    action(
                      'Animation tags…',
                      Icons.sell_outlined,
                      () => showDocumentTools(context, e, tab: 2),
                    ),
                    const Divider(),
                    Padding(
                      padding: const EdgeInsets.all(16),
                      child: Text(
                        current['linked'] != null
                            ? 'Cel linked to frame ${(current['linked'] as int) + 1}'
                            : 'Independent cel',
                      ),
                    ),
                    action(
                      'Link cel to previous frame',
                      Icons.link,
                      frame > 0 &&
                              current['group'] != true &&
                              current['locked'] != true
                          ? () => e.edits([
                              {'op': 'link-cel', 'source_frame': frame - 1},
                            ])
                          : null,
                    ),
                    action(
                      'Unlink cel',
                      Icons.link_off,
                      current['linked'] != null && current['locked'] != true
                          ? () => e.edits([
                              {'op': 'unlink-cel'},
                            ])
                          : null,
                    ),
                    action(
                      'Clear cel',
                      Icons.clear,
                      current['locked'] != true && current['group'] != true
                          ? () => e.edits([
                              {
                                'op': 'clear-cel-range',
                                'range': {
                                  'first_layer': layer,
                                  'last_layer': layer,
                                  'first_frame': frame,
                                  'last_frame': frame,
                                },
                              },
                            ])
                          : null,
                    ),
                    action(
                      'Delete frame',
                      Icons.delete_outline,
                      frames > 1
                          ? () => e.edits([
                              {'op': 'remove-frame'},
                            ])
                          : null,
                    ),
                  ],
                  if (section == 2) ...[
                    action('Add animation tag', Icons.add, () async {
                      final tag = await editAnimationTag(context, frames);
                      if (tag != null) {
                        await e.edits([
                          {'op': 'add-tag', 'tag': tag},
                        ]);
                      }
                    }),
                    if (tags.isEmpty)
                      const Padding(
                        padding: EdgeInsets.all(16),
                        child: Text(
                          'Name frame ranges such as idle, walk, jump or attack.',
                        ),
                      ),
                    for (final tag in tags.asMap().entries)
                      ListTile(
                        leading: IconButton(
                          tooltip: 'Play ${tag.value['name']}',
                          onPressed: enabled
                              ? () => e.startPlayback(tag: tag.key)
                              : null,
                          icon: const Icon(Icons.play_arrow),
                        ),
                        title: Text('${tag.value['name']}'),
                        subtitle: Text(
                          'Frames ${(tag.value['from'] as int) + 1}–${(tag.value['to'] as int) + 1}',
                        ),
                        onTap: enabled
                            ? () async {
                                final value = await editAnimationTag(
                                  context,
                                  frames,
                                  initial: Map<String, dynamic>.from(tag.value),
                                );
                                if (value != null) {
                                  await e.edits([
                                    {
                                      'op': 'set-tag',
                                      'index': tag.key,
                                      'tag': value,
                                    },
                                  ]);
                                }
                              }
                            : null,
                        trailing: IconButton(
                          tooltip: 'Delete ${tag.value['name']}',
                          onPressed: enabled
                              ? () => e.edits([
                                  {'op': 'remove-tag', 'index': tag.key},
                                ])
                              : null,
                          icon: const Icon(Icons.delete_outline),
                        ),
                      ),
                  ],
                  if (section == 3) ...[
                    const Padding(
                      padding: EdgeInsets.all(16),
                      child: Text(
                        'Drag Select on the canvas. Without a selection, actions use the whole canvas.',
                      ),
                    ),
                    for (final item in [
                      ('Select all', 'select-all', Icons.select_all),
                      ('Deselect', 'deselect', Icons.deselect),
                      ('Copy', 'copy', Icons.copy_outlined),
                      ('Cut', 'cut', Icons.content_cut),
                      ('Paste', 'paste', Icons.content_paste),
                      ('Delete pixels', 'delete', Icons.delete_outline),
                      ('Flip horizontally', 'flip-x', Icons.flip),
                      ('Flip vertically', 'flip-y', Icons.flip),
                      ('Rotate 90°', 'rotate', Icons.rotate_right),
                      ('Crop sprite to selection', 'crop', Icons.crop),
                    ])
                      action(
                        item.$1,
                        item.$3,
                        item.$2 == 'paste' && e.state['has_clipboard'] != true
                            ? null
                            : () => e.selectionAction(item.$2),
                      ),
                    action('Move selection', Icons.open_with, () async {
                      final dx = await askText(
                        context,
                        'Move horizontally (pixels)',
                        '0',
                        number: true,
                      );
                      if (dx == null || !context.mounted) return;
                      final dy = await askText(
                        context,
                        'Move vertically (pixels)',
                        '0',
                        number: true,
                      );
                      final x = int.tryParse(dx), y = int.tryParse(dy ?? '');
                      if (x != null && y != null) {
                        await e.selectionAction(
                          'move',
                          extra: {'dx': x, 'dy': y},
                        );
                      }
                    }),
                  ],
                ],
              ),
            ),
          ],
        ),
      );
    },
  );
}

class LayerRow extends StatelessWidget {
  final EditorController editor;
  final int index;
  const LayerRow({super.key, required this.editor, required this.index});
  @override
  Widget build(BuildContext context) {
    final layers = editor.state['layers'] as List? ?? [];
    final layer = layers[index] as Map;
    final name = '${layer['name']}';
    final enabled = editor.ready && !editor.busy && !editor.drawing;
    Future<void> change(Map<String, dynamic> command) async {
      await editor.edits([
        {'layer': index, ...command},
      ]);
    }

    return Material(
      color: editor.state['layer'] == index
          ? Theme.of(context).colorScheme.secondaryContainer
          : Colors.transparent,
      child: Row(
        children: [
          IconButton(
            tooltip: '${layer['visible'] == true ? 'Hide' : 'Show'} $name',
            onPressed: enabled
                ? () => change({
                    'op': 'set-layer',
                    'visible': layer['visible'] != true,
                  })
                : null,
            icon: Icon(
              layer['visible'] == true
                  ? Icons.visibility_outlined
                  : Icons.visibility_off_outlined,
            ),
          ),
          IconButton(
            tooltip: '${layer['locked'] == true ? 'Unlock' : 'Lock'} $name',
            onPressed: enabled
                ? () => change({
                    'op': 'set-layer',
                    'locked': layer['locked'] != true,
                  })
                : null,
            icon: Icon(
              layer['locked'] == true
                  ? Icons.lock_outline
                  : Icons.lock_open_outlined,
            ),
          ),
          Expanded(
            child: Semantics(
              selected: editor.state['layer'] == index,
              child: InkWell(
                onTap: enabled ? () => editor.command(10, a: index) : null,
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 4,
                    vertical: 12,
                  ),
                  child: Row(
                    children: [
                      Icon(
                        layer['group'] == true
                            ? Icons.folder_outlined
                            : Icons.layers_outlined,
                        size: 18,
                      ),
                      const SizedBox(width: 6),
                      Expanded(
                        child: Text(
                          name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
          PopupMenuButton<String>(
            tooltip: 'Layer options for $name',
            enabled: enabled,
            itemBuilder: (_) => [
              const PopupMenuItem(
                value: 'properties',
                child: Text('Properties…'),
              ),
              PopupMenuItem(
                value: 'front',
                enabled: index + 1 < layers.length,
                child: const Text('Move toward front'),
              ),
              PopupMenuItem(
                value: 'back',
                enabled: index > 0,
                child: const Text('Move toward back'),
              ),
              PopupMenuItem(
                value: 'delete',
                enabled: layers.length > 1 && layer['locked'] != true,
                child: const Text('Delete layer'),
              ),
            ],
            onSelected: (action) async {
              await editor.command(10, a: index);
              if (editor.error != null || !context.mounted) return;
              switch (action) {
                case 'properties':
                  await showDialog<void>(
                    context: context,
                    builder: (_) => _LayerProperties(editor: editor),
                  );
                case 'front':
                  await editor.edits([
                    {'op': 'move-layer', 'to': index + 1},
                  ]);
                case 'back':
                  await editor.edits([
                    {'op': 'move-layer', 'to': index - 1},
                  ]);
                case 'delete':
                  await editor.edits([
                    {'op': 'remove-layer'},
                  ]);
              }
            },
          ),
        ],
      ),
    );
  }
}

class _LayerProperties extends StatefulWidget {
  final EditorController editor;
  const _LayerProperties({required this.editor});
  @override
  State<_LayerProperties> createState() => _LayerPropertiesState();
}

class _LayerPropertiesState extends State<_LayerProperties> {
  late final Map layer =
      widget.editor.state['layers'][widget.editor.state['layer']] as Map;
  late final name = TextEditingController(text: '${layer['name']}');
  late double opacity = (layer['opacity'] as num?)?.toDouble() ?? 255;
  @override
  void dispose() {
    name.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: widget.editor,
    builder: (context, _) => AlertDialog(
      title: const Text('Layer properties'),
      content: SizedBox(
        width: 360,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: name,
                maxLength: 128,
                decoration: const InputDecoration(labelText: 'Name'),
              ),
              const SizedBox(height: 16),
              Text('Opacity ${(opacity / 255 * 100).round()}%'),
              Slider(
                value: opacity,
                min: 0,
                max: 255,
                divisions: 255,
                onChanged: (v) => setState(() => opacity = v),
              ),
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
          onPressed: widget.editor.busy || widget.editor.drawing
              ? null
              : () async {
                  if (name.text.trim().isEmpty) return;
                  await widget.editor.edits([
                    {
                      'op': 'set-layer',
                      'name': name.text.trim(),
                      'opacity': opacity.round(),
                    },
                  ]);
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

Future<Map<String, dynamic>?> editAnimationTag(
  BuildContext context,
  int frames, {
  Map<String, dynamic>? initial,
}) => showDialog<Map<String, dynamic>>(
  context: context,
  builder: (_) => _TagDialog(frames, initial),
);

class _TagDialog extends StatefulWidget {
  final int frames;
  final Map<String, dynamic>? initial;
  const _TagDialog(this.frames, this.initial);
  @override
  State<_TagDialog> createState() => _TagDialogState();
}

class _TagDialogState extends State<_TagDialog> {
  late final name = TextEditingController(
    text: widget.initial?['name'] as String? ?? 'idle',
  );
  late final from = TextEditingController(
    text: '${(widget.initial?['from'] as int? ?? 0) + 1}',
  );
  late final to = TextEditingController(
    text: '${(widget.initial?['to'] as int? ?? widget.frames - 1) + 1}',
  );
  late final repeat = TextEditingController(
    text: '${widget.initial?['repeat'] ?? 0}',
  );
  late int direction = widget.initial?['direction'] as int? ?? 0;
  String? error;
  @override
  void dispose() {
    name.dispose();
    from.dispose();
    to.dispose();
    repeat.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: const Text('Animation tag'),
    content: SizedBox(
      width: 340,
      child: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: name,
              maxLength: 32,
              decoration: const InputDecoration(labelText: 'Name'),
            ),
            TextField(
              controller: from,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'First frame'),
            ),
            TextField(
              controller: to,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Last frame'),
            ),
            DropdownButtonFormField<int>(
              initialValue: direction,
              isExpanded: true,
              decoration: const InputDecoration(labelText: 'Direction'),
              items: [
                for (final e in [
                  'Forward',
                  'Reverse',
                  'Ping-pong',
                  'Reverse ping-pong',
                ].asMap().entries)
                  DropdownMenuItem(value: e.key, child: Text(e.value)),
              ],
              onChanged: (v) => direction = v ?? 0,
            ),
            TextField(
              controller: repeat,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(
                labelText: 'Repeat count (0 = loop)',
              ),
            ),
            if (error != null)
              Text(
                error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
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
        onPressed: () {
          final a = int.tryParse(from.text),
              b = int.tryParse(to.text),
              r = int.tryParse(repeat.text);
          if (name.text.trim().isEmpty ||
              a == null ||
              b == null ||
              a < 1 ||
              b < a ||
              b > widget.frames ||
              r == null ||
              r < 0 ||
              r > 65535) {
            setState(
              () =>
                  error = 'Enter a name, a valid frame range and repeat count.',
            );
            return;
          }
          Navigator.pop(context, {
            'name': name.text.trim(),
            'from': a - 1,
            'to': b - 1,
            'direction': direction,
            'repeat': r,
            'color': widget.initial?['color'] ?? [64, 200, 180],
          });
        },
        child: const Text('Save tag'),
      ),
    ],
  );
}
