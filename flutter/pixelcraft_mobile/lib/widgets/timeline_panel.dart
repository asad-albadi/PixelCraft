import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../editor_controller.dart';
import 'document_tools.dart';

class TimelinePanel extends StatelessWidget {
  final EditorController editor;
  final bool allowExpanded;
  const TimelinePanel({
    super.key,
    required this.editor,
    required this.allowExpanded,
  });
  @override
  Widget build(BuildContext context) {
    final e = editor;
    final enabled = e.ready && !e.busy && !e.drawing;
    final frames = (e.state['frames'] as int?) ?? 1;
    final frame = (e.state['frame'] as int?) ?? 0;
    final layers = e.state['layers'] as List? ?? [];
    final expanded = allowExpanded && e.timelineExpanded;
    final textScale = MediaQuery.textScalerOf(context).scale(1);
    final cellWidth = math.max(76.0, 76 * textScale);
    final rowHeight = math.max(48.0, 24 * textScale + 16);
    final toolbar = SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: [
          IconButton(
            tooltip: e.playing ? 'Stop animation' : 'Play animation',
            onPressed: e.playing
                ? () => e.stopPlayback()
                : enabled
                ? () => e.startPlayback()
                : null,
            icon: Icon(e.playing ? Icons.stop : Icons.play_arrow),
          ),
          PopupMenuButton<double>(
            tooltip: 'Playback speed',
            enabled: enabled,
            initialValue: e.playbackSpeed,
            onSelected: e.setPlaybackSpeed,
            itemBuilder: (_) => [
              for (final v in [.25, .5, 1.0, 1.5, 2.0, 4.0])
                PopupMenuItem(value: v, child: Text('$v×')),
            ],
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Text('${e.playbackSpeed}× ▾'),
            ),
          ),
          TextButton.icon(
            onPressed: enabled
                ? () => showDocumentTools(context, e, tab: 1)
                : null,
            icon: const Icon(Icons.filter_none_outlined, size: 18),
            label: const Text('Frame…'),
          ),
          TextButton.icon(
            onPressed: enabled ? () => showDocumentTools(context, e) : null,
            icon: const Icon(Icons.layers_outlined, size: 18),
            label: const Text('Layer…'),
          ),
          IconButton(
            tooltip: 'Animation tags',
            onPressed: enabled
                ? () => showDocumentTools(context, e, tab: 2)
                : null,
            icon: const Icon(Icons.sell_outlined),
          ),
          IconButton(
            tooltip: 'Onion skin',
            isSelected: e.onionSkin,
            onPressed: enabled ? () => e.setOnionSkin(!e.onionSkin) : null,
            icon: const Icon(Icons.blur_on),
          ),
          if (allowExpanded)
            IconButton(
              tooltip: expanded ? 'Collapse timeline' : 'Expand timeline',
              onPressed: () => e.setTimelineExpanded(!e.timelineExpanded),
              icon: Icon(expanded ? Icons.expand_more : Icons.expand_less),
            ),
        ],
      ),
    );
    Widget headers() => SizedBox(
      height: math.max(60, 32 * textScale + 20),
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        itemCount: frames,
        separatorBuilder: (_, _) => const SizedBox(width: 6),
        itemBuilder: (context, index) => Semantics(
          label: 'Frame ${index + 1}',
          selected: (e.previewFrame ?? frame) == index,
          child: OutlinedButton(
            onPressed: enabled ? () => e.command(9, a: index) : null,
            onLongPress: enabled
                ? () async {
                    await e.command(9, a: index);
                    if (context.mounted && e.error == null) {
                      showDocumentTools(context, e, tab: 1);
                    }
                  }
                : null,
            style: OutlinedButton.styleFrom(
              minimumSize: const Size(64, 48),
              padding: const EdgeInsets.symmetric(horizontal: 12),
              backgroundColor: (e.previewFrame ?? frame) == index
                  ? Theme.of(context).colorScheme.primaryContainer
                  : null,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(6),
              ),
            ),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text('${index + 1}'),
                Text(
                  '${e.state['durations']?[index] ?? 100} ms',
                  style: const TextStyle(fontSize: 10),
                ),
              ],
            ),
          ),
        ),
      ),
    );
    return Material(
      color: Theme.of(context).colorScheme.surfaceContainerLow,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Divider(height: 1),
          toolbar,
          if (!expanded) headers(),
          if (expanded)
            SizedBox(
              height: math.min(
                240,
                rowHeight * layers.length + math.max(60, 32 * textScale + 20),
              ),
              child: SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: SizedBox(
                  width: 300 + frames * cellWidth,
                  child: Column(
                    children: [
                      SizedBox(
                        height: math.max(60, 32 * textScale + 20),
                        child: Row(
                          children: [
                            SizedBox(
                              width: 300,
                              child: Row(
                                children: [
                                  const SizedBox(width: 12),
                                  const Expanded(child: Text('Layers')),
                                  IconButton(
                                    tooltip: 'Add layer',
                                    onPressed: enabled && layers.length < 32
                                        ? () => e.addLayer()
                                        : null,
                                    icon: const Icon(Icons.add),
                                  ),
                                ],
                              ),
                            ),
                            for (var index = 0; index < frames; index++)
                              SizedBox(
                                width: cellWidth,
                                child: Padding(
                                  padding: const EdgeInsets.all(3),
                                  child: OutlinedButton(
                                    onPressed: enabled
                                        ? () => e.command(9, a: index)
                                        : null,
                                    onLongPress: enabled
                                        ? () async {
                                            await e.command(9, a: index);
                                            if (context.mounted &&
                                                e.error == null) {
                                              showDocumentTools(
                                                context,
                                                e,
                                                tab: 1,
                                              );
                                            }
                                          }
                                        : null,
                                    style: OutlinedButton.styleFrom(
                                      padding: EdgeInsets.zero,
                                      backgroundColor:
                                          (e.previewFrame ?? frame) == index
                                          ? Theme.of(context)
                                                .colorScheme
                                                .primaryContainer
                                          : null,
                                    ),
                                    child: Column(
                                      mainAxisAlignment:
                                          MainAxisAlignment.center,
                                      children: [
                                        Text('${index + 1}'),
                                        Text(
                                          '${e.state['durations']?[index] ?? 100} ms',
                                          style: const TextStyle(fontSize: 10),
                                        ),
                                      ],
                                    ),
                                  ),
                                ),
                              ),
                          ],
                        ),
                      ),
                      Expanded(
                        child: ListView(
                          children: [
                            for (final item
                                in layers.asMap().entries.toList().reversed)
                              SizedBox(
                                height: rowHeight,
                                child: Row(
                                  children: [
                                    SizedBox(
                                      width: 300,
                                      child: LayerRow(
                                        editor: e,
                                        index: item.key,
                                      ),
                                    ),
                                    for (var f = 0; f < frames; f++)
                                      SizedBox(
                                        width: cellWidth,
                                        child: Padding(
                                          padding: const EdgeInsets.all(3),
                                          child: Semantics(
                                            selected:
                                                e.state['layer'] == item.key &&
                                                frame == f,
                                            child: IconButton(
                                              tooltip:
                                                  '${item.value['name']}, frame ${f + 1}',
                                              onPressed: enabled
                                                  ? () async {
                                                      await e.command(
                                                        10,
                                                        a: item.key,
                                                      );
                                                      if (e.error == null) {
                                                        await e.command(
                                                          9,
                                                          a: f,
                                                        );
                                                      }
                                                    }
                                                  : null,
                                              style: IconButton.styleFrom(
                                                backgroundColor:
                                                    e.state['layer'] ==
                                                            item.key &&
                                                        frame == f
                                                    ? Theme.of(context)
                                                          .colorScheme
                                                          .primaryContainer
                                                    : null,
                                                minimumSize: const Size(48, 48),
                                              ),
                                              icon: Icon(
                                                item.value['group'] == true
                                                    ? Icons.folder_outlined
                                                    : f == frame &&
                                                          item.value['linked'] !=
                                                              null
                                                    ? Icons.link
                                                    : Icons.crop_square,
                                                size: 16,
                                              ),
                                            ),
                                          ),
                                        ),
                                      ),
                                  ],
                                ),
                              ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}
