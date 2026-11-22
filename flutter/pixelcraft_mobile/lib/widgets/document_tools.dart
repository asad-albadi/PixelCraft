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
  constraints: const BoxConstraints(maxWidth: 720),
  builder: (_) => SizedBox(
    height: MediaQuery.sizeOf(context).height * .82,
    child: _DocumentTools(editor, tab),
  ),
);

class _DocumentTools extends StatefulWidget {
  final EditorController editor;
  final int tab;
  const _DocumentTools(this.editor, this.tab);
  @override
  State<_DocumentTools> createState() => _DocumentToolsState();
}

class _DocumentToolsState extends State<_DocumentTools> {
  double? opacity;
  EditorController get e => widget.editor;
  Widget action(String label, IconData icon, VoidCallback? call) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 3),
    child: OutlinedButton.icon(
      onPressed: e.busy || e.drawing ? null : call,
      icon: Icon(icon, size: 19),
      label: Text(label),
    ),
  );
  @override
  Widget build(BuildContext context) => DefaultTabController(
    length: 4,
    initialIndex: widget.tab,
    child: AnimatedBuilder(
      animation: e,
      builder: (c, _) {
        final layer = (e.state['layer'] as int?) ?? 0,
            frame = (e.state['frame'] as int?) ?? 0;
        final layers = e.state['layers'] as List? ?? [];
        final frames = (e.state['frames'] as int?) ?? 1;
        final current = layers.isNotEmpty
            ? layers[layer] as Map
            : <String, dynamic>{};
        final tags = e.state['tags'] as List? ?? [];
        return SafeArea(
          child: Column(
            children: [
              const Padding(
                padding: EdgeInsets.symmetric(horizontal: 16),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    'Sprite tools',
                    style: TextStyle(fontSize: 20, fontWeight: FontWeight.w600),
                  ),
                ),
              ),
              const TabBar(
                isScrollable: true,
                tabs: [
                  Tab(text: 'Layers'),
                  Tab(text: 'Frames'),
                  Tab(text: 'Tags'),
                  Tab(text: 'Selection'),
                ],
              ),
              Expanded(
                child: TabBarView(
                  children: [
                    ListView(
                      padding: const EdgeInsets.all(20),
                      children: [
                        DropdownButton<int>(
                          isExpanded: true,
                          value: layers.isEmpty ? null : layer,
                          items: [
                            for (final item in layers.asMap().entries)
                              DropdownMenuItem(
                                value: item.key,
                                child: Text(
                                  '${item.value['group'] == true ? 'Group: ' : ''}${item.value['name']}',
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                          ],
                          onChanged: e.busy
                              ? null
                              : (n) {
                                  if (n != null) e.command(10, a: n);
                                },
                        ),
                        Wrap(
                          spacing: 8,
                          children: [
                            action(
                              'Add layer',
                              Icons.add,
                              layers.length < 32 ? () => e.addLayer() : null,
                            ),
                            action(
                              'Add group',
                              Icons.create_new_folder_outlined,
                              layers.length < 32
                                  ? () => e.addLayer(group: true)
                                  : null,
                            ),
                          ],
                        ),
                        action('Rename layer', Icons.edit_outlined, () async {
                          final name = await askText(
                            context,
                            'Layer name',
                            current['name'] as String? ?? 'Layer',
                          );
                          if (name != null && name.trim().isNotEmpty) {
                            await e.edits([
                              {'op': 'set-layer', 'name': name.trim()},
                            ]);
                          }
                        }),
                        SwitchListTile(
                          title: const Text('Visible'),
                          value: current['visible'] == true,
                          onChanged: e.busy
                              ? null
                              : (v) => e.edits([
                                  {'op': 'set-layer', 'visible': v},
                                ]),
                        ),
                        SwitchListTile(
                          title: const Text('Locked'),
                          value: current['locked'] == true,
                          onChanged: e.busy
                              ? null
                              : (v) => e.edits([
                                  {'op': 'set-layer', 'locked': v},
                                ]),
                        ),
                        Text(
                          'Opacity ${((opacity ?? ((current['opacity'] as num?)?.toDouble() ?? 255)) / 255 * 100).round()}%',
                        ),
                        Slider(
                          value:
                              opacity ??
                              ((current['opacity'] as num?)?.toDouble() ?? 255),
                          min: 0,
                          max: 255,
                          divisions: 255,
                          onChanged: e.busy
                              ? null
                              : (v) => setState(() => opacity = v),
                          onChangeEnd: (v) async {
                            await e.edits([
                              {'op': 'set-layer', 'opacity': v.round()},
                            ]);
                            if (mounted) setState(() => opacity = null);
                          },
                        ),
                        action(
                          'Move toward front',
                          Icons.arrow_upward,
                          layer + 1 < layers.length
                              ? () => e.edits([
                                  {'op': 'move-layer', 'to': layer + 1},
                                ])
                              : null,
                        ),
                        action(
                          'Move toward back',
                          Icons.arrow_downward,
                          layer > 0
                              ? () => e.edits([
                                  {'op': 'move-layer', 'to': layer - 1},
                                ])
                              : null,
                        ),
                        action(
                          'Delete layer',
                          Icons.delete_outline,
                          layers.length > 1 && current['locked'] != true
                              ? () => e.edits([
                                  {'op': 'remove-layer'},
                                ])
                              : null,
                        ),
                      ],
                    ),
                    ListView(
                      padding: const EdgeInsets.all(20),
                      children: [
                        Text(
                          'Frame ${frame + 1} of $frames',
                          style: Theme.of(context).textTheme.titleMedium,
                        ),
                        SwitchListTile(
                          title: const Text('Onion skin'),
                          subtitle: const Text(
                            'Show adjacent frames on the canvas',
                          ),
                          value: e.onionSkin,
                          onChanged: e.busy ? null : (v) => e.setOnionSkin(v),
                        ),
                        Wrap(
                          spacing: 8,
                          children: [
                            action(
                              'Previous frame',
                              Icons.skip_previous,
                              frame > 0
                                  ? () => e.command(9, a: frame - 1)
                                  : null,
                            ),
                            action(
                              'Next frame',
                              Icons.skip_next,
                              frame + 1 < frames
                                  ? () => e.command(9, a: frame + 1)
                                  : null,
                            ),
                          ],
                        ),
                        action(
                          'Add blank frame',
                          Icons.add,
                          frames < 64 ? () => e.addFrame() : null,
                        ),
                        action(
                          'Duplicate frame',
                          Icons.copy_outlined,
                          frames < 64
                              ? () => e.addFrame(duplicate: true)
                              : null,
                        ),
                        action(
                          'Frame duration',
                          Icons.timer_outlined,
                          () async {
                            final text = await askText(
                              context,
                              'Duration in milliseconds',
                              '${e.state['durations'][frame]}',
                              number: true,
                            );
                            final n = int.tryParse(text ?? '');
                            if (n != null) {
                              await e.edits([
                                {'op': 'set-frame', 'duration': n},
                              ]);
                            }
                          },
                        ),
                        action('Frame name', Icons.label_outline, () async {
                          final names = e.state['frame_names'] as List? ?? [];
                          final text = await askText(
                            context,
                            'Frame name',
                            frame < names.length ? names[frame] as String : '',
                          );
                          if (text != null) {
                            await e.edits([
                              {'op': 'set-frame', 'name': text},
                            ]);
                          }
                        }),
                        const Divider(),
                        Text(
                          current['linked'] != null
                              ? 'Cel linked to frame ${(current['linked'] as int) + 1}'
                              : 'Independent cel',
                        ),
                        action(
                          'Link cel to previous frame',
                          Icons.link,
                          frame > 0 && current['group'] != true
                              ? () => e.edits([
                                  {'op': 'link-cel', 'source_frame': frame - 1},
                                ])
                              : null,
                        ),
                        action(
                          'Unlink cel',
                          Icons.link_off,
                          current['group'] != true
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
                    ),
                    ListView(
                      padding: const EdgeInsets.all(20),
                      children: [
                        const Text(
                          'Name animation ranges such as idle, walk, jump or attack.',
                        ),
                        action('Add animation tag', Icons.add, () async {
                          final tag = await editAnimationTag(context, frames);
                          if (tag != null) {
                            await e.edits([
                              {'op': 'add-tag', 'tag': tag},
                            ]);
                          }
                        }),
                        for (final tag in tags.asMap().entries)
                          ListTile(
                            leading: IconButton(
                              tooltip: 'Play tag',
                              onPressed: e.busy
                                  ? null
                                  : () => e.startPlayback(tag: tag.key),
                              icon: const Icon(Icons.play_arrow),
                            ),
                            title: Text(tag.value['name'] as String),
                            subtitle: Text(
                              'Frames ${(tag.value['from'] as int) + 1}–${(tag.value['to'] as int) + 1}',
                            ),
                            onTap: e.busy
                                ? null
                                : () async {
                                    final value = await editAnimationTag(
                                      context,
                                      frames,
                                      initial: Map<String, dynamic>.from(
                                        tag.value,
                                      ),
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
                                  },
                            trailing: IconButton(
                              tooltip: 'Delete tag',
                              onPressed: e.busy
                                  ? null
                                  : () => e.edits([
                                      {'op': 'remove-tag', 'index': tag.key},
                                    ]),
                              icon: const Icon(Icons.delete_outline),
                            ),
                          ),
                      ],
                    ),
                    ListView(
                      padding: const EdgeInsets.all(20),
                      children: [
                        const Text(
                          'Use Select to drag a rectangle on the canvas. With no selection, these actions use the whole canvas.',
                        ),
                        for (final item in [
                          ('Select all', 'select-all', Icons.select_all),
                          ('Deselect', 'deselect', Icons.deselect),
                          ('Copy', 'copy', Icons.copy_outlined),
                          ('Cut', 'cut', Icons.content_cut_outlined),
                          ('Paste', 'paste', Icons.content_paste_outlined),
                          ('Delete pixels', 'delete', Icons.delete_outline),
                          ('Flip horizontally', 'flip-x', Icons.flip),
                          ('Flip vertically', 'flip-y', Icons.flip),
                          ('Rotate 90°', 'rotate', Icons.rotate_right),
                          ('Crop sprite to selection', 'crop', Icons.crop),
                        ])
                          action(
                            item.$1,
                            item.$3,
                            item.$2 == 'paste' &&
                                    e.state['has_clipboard'] != true
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
                          final x = int.tryParse(dx),
                              y = int.tryParse(dy ?? '');
                          if (x != null && y != null) {
                            await e.selectionAction(
                              'move',
                              extra: {'dx': x, 'dy': y},
                            );
                          }
                        }),
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
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
