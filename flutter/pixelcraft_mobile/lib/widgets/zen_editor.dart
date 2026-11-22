import 'package:flutter/material.dart';

import '../editor_controller.dart';
import 'color_dialog.dart';
import 'pixel_canvas.dart';
import 'document_tools.dart';

/// Canvas-only presentation; owns no artwork or history.
class ZenEditor extends StatelessWidget {
  final EditorController editor;
  const ZenEditor({super.key, required this.editor});
  static const tools = [
    ('Draw', Icons.edit_outlined),
    ('Erase', Icons.auto_fix_normal_outlined),
    ('Fill', Icons.format_color_fill_outlined),
    ('Eyedropper', Icons.colorize_outlined),
    ('Line', Icons.show_chart),
    ('Rectangle', Icons.crop_square),
    ('Ellipse', Icons.circle_outlined),
    ('Select', Icons.check_box_outline_blank),
    ('Pan', Icons.pan_tool_outlined),
  ];
  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: LayoutBuilder(
        builder: (context, box) {
          final compact = box.maxWidth < 600 || box.maxHeight < 500;
          final enabled = editor.ready && !editor.busy && !editor.drawing;
          Widget button(
            String label,
            IconData icon,
            VoidCallback? call, {
            bool selected = false,
          }) => IconButton(
            tooltip: label,
            isSelected: selected,
            onPressed: call,
            icon: Icon(icon),
            style: IconButton.styleFrom(
              backgroundColor: selected
                  ? Theme.of(context).colorScheme.primaryContainer
                  : Colors.transparent,
            ),
          );
          final controls = <Widget>[
            button(
              'Undo',
              Icons.undo_rounded,
              editor.canUndo ? () => editor.command(3) : null,
            ),
            button(
              'Redo',
              Icons.redo_rounded,
              editor.canRedo ? () => editor.command(4) : null,
            ),
            button(
              'Pencil',
              Icons.edit_outlined,
              enabled ? () => editor.setTool('Draw') : null,
              selected: editor.tool == 'Draw',
            ),
            button(
              'Eraser',
              Icons.auto_fix_normal_outlined,
              enabled ? () => editor.setTool('Erase') : null,
              selected: editor.tool == 'Erase',
            ),
            IconButton(
              tooltip: 'Choose color',
              onPressed: enabled
                  ? () async {
                      final color = await editColor(context, editor.color);
                      if (color != null) {
                        await editor.addNamedColor(color.$1, color.$2);
                      }
                    }
                  : null,
              icon: Container(
                width: 24,
                height: 24,
                decoration: BoxDecoration(
                  color: Color(editor.color),
                  borderRadius: BorderRadius.circular(5),
                  border: Border.all(
                    color: Theme.of(context).colorScheme.outline,
                  ),
                ),
              ),
            ),
            PopupMenuButton<int>(
              tooltip: 'Brush size',
              enabled: enabled,
              initialValue: editor.brush,
              itemBuilder: (_) => [
                for (final n in [1, 2, 4, 8, 16, 32])
                  PopupMenuItem(value: n, child: Text('$n px')),
              ],
              onSelected: editor.setBrush,
              child: Tooltip(
                message: 'Brush size: ${editor.brush} pixels',
                child: SizedBox(
                  width: 48,
                  height: 48,
                  child: Center(
                    child: Text(
                      '${editor.brush}',
                      style: const TextStyle(fontSize: 14),
                    ),
                  ),
                ),
              ),
            ),
            PopupMenuButton<String>(
              tooltip: 'More drawing controls',
              enabled: enabled,
              itemBuilder: (_) => [
                for (final tool in tools)
                  CheckedPopupMenuItem(
                    value: 'tool:${tool.$1}',
                    checked: editor.tool == tool.$1,
                    child: Text(tool.$1),
                  ),
                const PopupMenuDivider(),
                const PopupMenuItem(value: 'fit', child: Text('Fit canvas')),
                const PopupMenuItem(
                  value: 'layers',
                  child: Text('Layer options…'),
                ),
                const PopupMenuItem(
                  value: 'frames',
                  child: Text('Frame options…'),
                ),
                const PopupMenuItem(value: 'save', child: Text('Save')),
                if (!compact)
                  PopupMenuItem(
                    value: 'side',
                    child: Text(
                      editor.zenControlsLeft
                          ? 'Move controls right'
                          : 'Move controls left',
                    ),
                  ),
              ],
              onSelected: (action) {
                if (action.startsWith('tool:')) {
                  editor.setTool(action.substring(5));
                  return;
                }
                switch (action) {
                  case 'fit':
                    editor.fitCanvas();
                  case 'layers':
                    showDocumentTools(context, editor, tab: 0);
                  case 'frames':
                    showDocumentTools(context, editor, tab: 1);
                  case 'save':
                    editor.save();
                  case 'side':
                    editor.setZenControlsLeft(!editor.zenControlsLeft);
                }
              },
            ),
          ];
          final strip = GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () {},
            child: Material(
              elevation: 6,
              color: Theme.of(context).colorScheme.surfaceContainer,
              borderRadius: BorderRadius.circular(16),
              clipBehavior: Clip.antiAlias,
              child: Padding(
                padding: const EdgeInsets.all(4),
                child: compact
                    ? Wrap(alignment: WrapAlignment.center, children: controls)
                    : Column(
                        mainAxisSize: MainAxisSize.min,
                        children: controls,
                      ),
              ),
            ),
          );
          return Stack(
            children: [
              Positioned.fill(
                child: RepaintBoundary(child: PixelCanvas(editor: editor)),
              ),
              Positioned(
                top: 12,
                left: 12,
                child: FilledButton.tonalIcon(
                  key: const Key('exit-zen'),
                  onPressed: editor.exitZen,
                  icon: const Icon(Icons.fullscreen_exit),
                  label: const Text('Exit Zen'),
                ),
              ),
              if (compact)
                Positioned(
                  bottom: 12,
                  left: 12,
                  right: 12,
                  child: Center(child: strip),
                )
              else
                Positioned(
                  top: 0,
                  bottom: 0,
                  left: editor.zenControlsLeft ? 12 : null,
                  right: editor.zenControlsLeft ? null : 12,
                  child: Center(child: strip),
                ),
              if (editor.loadingFile)
                const Positioned(
                  top: 0,
                  left: 0,
                  right: 0,
                  child: LinearProgressIndicator(minHeight: 2),
                ),
              if (editor.error != null)
                Positioned(
                  top: 68,
                  left: 12,
                  right: 12,
                  child: Material(
                    color: Theme.of(context).colorScheme.errorContainer,
                    borderRadius: BorderRadius.circular(8),
                    child: Padding(
                      padding: const EdgeInsets.only(left: 12),
                      child: Row(
                        children: [
                          Expanded(child: Text(editor.error!)),
                          IconButton(
                            tooltip: 'Dismiss message',
                            onPressed: editor.dismissError,
                            icon: const Icon(Icons.close),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
            ],
          );
        },
      ),
    ),
  );
}
