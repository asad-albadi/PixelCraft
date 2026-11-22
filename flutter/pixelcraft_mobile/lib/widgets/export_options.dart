import 'package:flutter/material.dart';

import 'dart:async';

import '../editor_controller.dart';

void showExportOptions(BuildContext context, EditorController editor) {
  unawaited(editor.loadExportFolder());
  showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    constraints: const BoxConstraints(maxWidth: 640),
    builder: (_) => SafeArea(
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight: MediaQuery.sizeOf(context).height * .8,
        ),
        child: AnimatedBuilder(
          animation: editor,
          builder: (context, _) => ListView(
            shrinkWrap: true,
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              Text(
                'Export & share',
                style: Theme.of(context).textTheme.titleLarge,
              ),
              const Padding(
                padding: EdgeInsets.symmetric(vertical: 12),
                child: Text(
                  'Export a copy; your editable project stays open. For a sprite sheet, save PNG first and then JSON to the chosen folder. JSON uses the actual saved PNG name.',
                ),
              ),
              ListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Export location'),
                subtitle: Text(
                  editor.exportFolder ?? 'Ask where to save each file',
                ),
                trailing: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    IconButton(
                      tooltip: 'Choose export folder',
                      onPressed: editor.busy ? null : editor.chooseExportFolder,
                      icon: const Icon(Icons.create_new_folder_outlined),
                    ),
                    if (editor.exportFolder != null)
                      IconButton(
                        tooltip: 'Forget export folder',
                        onPressed: editor.busy
                            ? null
                            : editor.clearExportFolder,
                        icon: const Icon(Icons.close),
                      ),
                  ],
                ),
              ),
              for (final entry in [
                (1, 'PNG image', 'Current frame; full transparency'),
                (
                  2,
                  'Animated GIF',
                  'All frames; GIF uses 10 ms timing and reduced colors',
                ),
                (
                  3,
                  'Aseprite project',
                  'Editable supported layers, frames and tags',
                ),
                (4, 'Sprite sheet PNG', 'All frames in fixed cells'),
                (
                  5,
                  'Sprite sheet JSON',
                  'Matching names, bounds, durations and tags',
                ),
              ])
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  title: Text(entry.$2),
                  subtitle: Text(entry.$3),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      IconButton(
                        tooltip: 'Save ${entry.$2}',
                        onPressed: editor.busy || editor.drawing
                            ? null
                            : () => editor.exportArtwork(entry.$1),
                        icon: const Icon(Icons.save_alt),
                      ),
                      IconButton(
                        tooltip: 'Share ${entry.$2}',
                        onPressed: editor.busy || editor.drawing
                            ? null
                            : () => editor.exportArtwork(entry.$1, share: true),
                        icon: const Icon(Icons.share_outlined),
                      ),
                    ],
                  ),
                ),
              if (editor.loadingFile) const LinearProgressIndicator(),
              Padding(
                padding: const EdgeInsets.only(top: 12),
                child: Text(
                  editor.error ?? editor.status,
                  style: TextStyle(
                    color: editor.error != null
                        ? Theme.of(context).colorScheme.error
                        : null,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    ),
  );
}
