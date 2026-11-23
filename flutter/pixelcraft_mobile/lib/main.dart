import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'editor_controller.dart';
import 'widgets/color_dialog.dart';
import 'widgets/document_tools.dart';
import 'widgets/pixel_canvas.dart';
import 'widgets/zen_editor.dart';
import 'widgets/timeline_panel.dart';
import 'widgets/export_options.dart';
import 'widgets/advanced_tools.dart';
import 'widgets/privacy_policy.dart';
import 'widgets/editor_keyboard.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const PixelCraftApp());
}

class PixelCraftApp extends StatefulWidget {
  final EditorController? controller;
  const PixelCraftApp({super.key, this.controller});
  @override
  State<PixelCraftApp> createState() => _PixelCraftAppState();
}

class _PixelCraftAppState extends State<PixelCraftApp>
    with WidgetsBindingObserver {
  late final EditorController editor = widget.controller ?? EditorController();
  bool dark = true;
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    if (widget.controller == null) unawaited(editor.initialize());
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state != AppLifecycleState.resumed) {
      unawaited(editor.pause());
    } else {
      editor.resume();
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    if (widget.controller == null) editor.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnnotatedRegion<SystemUiOverlayStyle>(
    value: dark ? SystemUiOverlayStyle.light : SystemUiOverlayStyle.dark,
    child: MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'PixelCraft',
      theme: ThemeData(
        useMaterial3: true,
        brightness: dark ? Brightness.dark : Brightness.light,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xff008585),
          brightness: dark ? Brightness.dark : Brightness.light,
        ),
        scaffoldBackgroundColor: dark
            ? const Color(0xff202227)
            : const Color(0xfff6f7fa),
        textTheme: const TextTheme(
          bodyMedium: TextStyle(fontSize: 14),
          bodySmall: TextStyle(fontSize: 12),
        ),
      ),
      home: EditorKeyboard(
        editor: editor,
        child: EditorScreen(
          editor: editor,
          toggleTheme: () => setState(() => dark = !dark),
        ),
      ),
    ),
  );
}

class EditorScreen extends StatelessWidget {
  final EditorController editor;
  final VoidCallback toggleTheme;
  const EditorScreen({
    super.key,
    required this.editor,
    required this.toggleTheme,
  });
  Future<bool> _discard(BuildContext c) async {
    if (!editor.dirty) return true;
    final discard =
        await showDialog<bool>(
          context: c,
          builder: (c) => AlertDialog(
            title: const Text('Keep your changes?'),
            content: const Text(
              'Save your artwork before opening another canvas.',
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.pop(c, false),
                child: const Text('Keep editing'),
              ),
              TextButton(
                onPressed: () => Navigator.pop(c, true),
                child: const Text('Discard changes'),
              ),
            ],
          ),
        ) ??
        false;
    return discard && await editor.discardRecovery();
  }

  void _inspect(BuildContext c) => showModalBottomSheet<void>(
    context: c,
    isScrollControlled: true,
    showDragHandle: true,
    builder: (_) => SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(c).height * .75,
        child: AnimatedBuilder(
          animation: editor,
          builder: (_, _) => Inspector(editor: editor),
        ),
      ),
    ),
  );
  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: editor,
    builder: (context, _) {
      final screen = MediaQuery.sizeOf(context);
      final compact =
          screen.width / MediaQuery.textScalerOf(context).scale(1) < 600 ||
          screen.height < 500;
      final available = editor.ready && !editor.busy && !editor.drawing;
      return PopScope(
        canPop: !editor.zen && !editor.dirty && !editor.drawing && !editor.busy,
        onPopInvokedWithResult: (didPop, _) async {
          if (didPop) return;
          if (editor.importing) {
            editor.cancelImport();
            return;
          }
          if (editor.zen) {
            await editor.exitZen();
            return;
          }
          if (editor.playing) {
            await editor.stopPlayback();
            return;
          }
          if (editor.busy) return;
          await editor.cancel();
          if (!context.mounted) return;
          if (await _discard(context)) await SystemNavigator.pop();
        },
        child: editor.zen
            ? ZenEditor(editor: editor)
            : Scaffold(
                body: SafeArea(
                  child: Column(
                    children: [
                      Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 8,
                          vertical: 6,
                        ),
                        child: Row(
                          children: [
                            Image.asset(
                              'assets/branding/pixelcraft.png',
                              width: 28,
                              height: 28,
                              filterQuality: FilterQuality.none,
                              excludeFromSemantics: true,
                            ),
                            const SizedBox(width: 8),
                            const Expanded(
                              child: Text(
                                'PixelCraft',
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(
                                  fontSize: 19,
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                            ),
                            IconButton(
                              tooltip: 'Undo',
                              onPressed: editor.canUndo
                                  ? () => editor.command(3)
                                  : null,
                              icon: const Icon(Icons.undo_rounded),
                            ),
                            IconButton(
                              tooltip: 'Redo',
                              onPressed: editor.canRedo
                                  ? () => editor.command(4)
                                  : null,
                              icon: const Icon(Icons.redo_rounded),
                            ),
                            if (compact)
                              IconButton(
                                key: const Key('save'),
                                tooltip: 'Save',
                                onPressed: available
                                    ? () => editor.save()
                                    : null,
                                icon: const Icon(Icons.save_outlined),
                              )
                            else
                              FilledButton.icon(
                                key: const Key('save'),
                                onPressed: available
                                    ? () => editor.save()
                                    : null,
                                icon: const Icon(Icons.save_outlined, size: 19),
                                label: const Text('Save'),
                              ),
                            if (!compact)
                              IconButton(
                                tooltip: 'Zen drawing mode',
                                onPressed: available
                                    ? () => editor.setZen(true)
                                    : null,
                                icon: const Icon(Icons.fullscreen),
                              ),
                            if (!compact)
                              IconButton(
                                tooltip: 'Change appearance',
                                onPressed: toggleTheme,
                                icon: const Icon(Icons.brightness_6_outlined),
                              ),
                            PopupMenuButton<String>(
                              tooltip: 'File actions',
                              onSelected: (action) async {
                                if (action == 'privacy') {
                                  await showPrivacyPolicy(context);
                                  return;
                                }
                                if (action == 'export') {
                                  showExportOptions(context, editor);
                                  return;
                                }
                                if (action == 'advanced') {
                                  showAdvancedTools(context, editor);
                                  return;
                                }
                                if (action == 'copy') {
                                  await editor.save(copy: true);
                                  return;
                                }
                                if (action == 'zen') {
                                  editor.setZen(true);
                                  return;
                                }
                                if (action == 'theme') {
                                  toggleTheme();
                                  return;
                                }
                                if (!await _discard(context)) return;
                                switch (action) {
                                  case 'new':
                                    await editor.newDocument();
                                  case 'demo':
                                    await editor.newDocument(demo: true);
                                  case 'open':
                                    await editor.open(picker: true);
                                  case 'reopen':
                                    await editor.open();
                                }
                              },
                              itemBuilder: (_) => [
                                for (final entry in [
                                  ('zen', 'Zen drawing mode', Icons.fullscreen),
                                  (
                                    'export',
                                    'Export & share…',
                                    Icons.ios_share,
                                  ),
                                  ('advanced', 'Document tools…', Icons.tune),
                                  (
                                    'new',
                                    'New 64 × 64 sprite',
                                    Icons.note_add_outlined,
                                  ),
                                  (
                                    'open',
                                    'Open sprite or image…',
                                    Icons.folder_open_outlined,
                                  ),
                                  (
                                    'reopen',
                                    'Reopen saved project',
                                    Icons.history_rounded,
                                  ),
                                  (
                                    'copy',
                                    'Save a copy…',
                                    Icons.file_upload_outlined,
                                  ),
                                  (
                                    'demo',
                                    'Open demo artwork',
                                    Icons.landscape_outlined,
                                  ),
                                  (
                                    'theme',
                                    'Change appearance',
                                    Icons.brightness_6_outlined,
                                  ),
                                  (
                                    'privacy',
                                    'Privacy policy',
                                    Icons.privacy_tip_outlined,
                                  ),
                                ])
                                  PopupMenuItem(
                                    value: entry.$1,
                                    enabled:
                                        available ||
                                        entry.$1 == 'theme' ||
                                        entry.$1 == 'privacy',
                                    child: Row(
                                      children: [
                                        Icon(entry.$3, size: 20),
                                        const SizedBox(width: 12),
                                        Expanded(child: Text(entry.$2)),
                                      ],
                                    ),
                                  ),
                              ],
                            ),
                          ],
                        ),
                      ),
                      if (editor.importing)
                        MaterialBanner(
                          content: const Text(
                            'Preparing import. Your current artwork stays open until it is ready.',
                          ),
                          actions: [
                            TextButton(
                              onPressed: editor.cancelImport,
                              child: const Text('Cancel import'),
                            ),
                          ],
                        ),
                      if (editor.recoveryPending)
                        MaterialBanner(
                          content: const Text(
                            'Unsaved artwork was found. Recover it or discard this recovery copy.',
                          ),
                          actions: [
                            TextButton(
                              onPressed: editor.recoveryResolving
                                  ? null
                                  : () =>
                                        editor.resolveRecovery(restore: false),
                              child: const Text('Discard recovery'),
                            ),
                            FilledButton(
                              onPressed:
                                  (editor.recoveryResolving ||
                                      !editor.canRestoreRecovery)
                                  ? null
                                  : () => editor.resolveRecovery(restore: true),
                              child: const Text('Recover artwork'),
                            ),
                          ],
                        ),
                      const Divider(height: 1),
                      SizedBox(
                        height: 2,
                        child: editor.loadingFile
                            ? const LinearProgressIndicator(minHeight: 2)
                            : null,
                      ),
                      if (editor.error != null)
                        Material(
                          color: Theme.of(context).colorScheme.errorContainer,
                          child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 12),
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
                      Expanded(
                        child: LayoutBuilder(
                          builder: (context, box) {
                            final wide =
                                box.maxWidth >= 850 && box.maxHeight >= 480;
                            return Row(
                              children: [
                                if (wide)
                                  SizedBox(
                                    width: 224,
                                    child: Inspector(editor: editor),
                                  ),
                                if (wide) const VerticalDivider(width: 1),
                                Expanded(
                                  child: Column(
                                    children: [
                                      Container(
                                        height: 48,
                                        padding: const EdgeInsets.symmetric(
                                          horizontal: 12,
                                        ),
                                        child: Row(
                                          children: [
                                            const Icon(
                                              Icons.grid_view_outlined,
                                              size: 17,
                                            ),
                                            const SizedBox(width: 8),
                                            Expanded(
                                              child: Text(
                                                '${editor.title}${editor.dirty ? ' •' : ''}',
                                                overflow: TextOverflow.ellipsis,
                                              ),
                                            ),
                                            if (!compact)
                                              Text(
                                                '${editor.width} × ${editor.height}',
                                                style: Theme.of(context)
                                                    .textTheme
                                                    .bodySmall,
                                              ),
                                            if (!wide)
                                              IconButton(
                                                tooltip: 'Colors and layers',
                                                onPressed: () =>
                                                    _inspect(context),
                                                icon: Icon(
                                                  Icons.palette_outlined,
                                                  color: Color(editor.color),
                                                ),
                                              ),
                                            PopupMenuButton<int>(
                                              tooltip: 'Brush size',
                                              initialValue: editor.brush,
                                              onSelected: editor.setBrush,
                                              itemBuilder: (_) =>
                                                  [1, 2, 4, 8, 16]
                                                      .map(
                                                        (n) => PopupMenuItem(
                                                          value: n,
                                                          child: Text('$n px'),
                                                        ),
                                                      )
                                                      .toList(),
                                              child: Padding(
                                                padding: const EdgeInsets.all(
                                                  12,
                                                ),
                                                child: Text(
                                                  '${editor.brush} px ▾',
                                                ),
                                              ),
                                            ),
                                          ],
                                        ),
                                      ),
                                      Expanded(
                                        child: RepaintBoundary(
                                          child: RepaintBoundary(
                                            child: PixelCanvas(editor: editor),
                                          ),
                                        ),
                                      ),
                                      TimelinePanel(
                                        editor: editor,
                                        allowExpanded:
                                            wide && box.maxHeight >= 480,
                                      ),
                                    ],
                                  ),
                                ),
                                const VerticalDivider(width: 1),
                                SizedBox(
                                  width: 60,
                                  child: ListView(
                                    children: [
                                      const SizedBox(height: 8),
                                      for (final tool in [
                                        (Icons.edit_outlined, 'Draw'),
                                        (
                                          Icons.auto_fix_normal_outlined,
                                          'Erase',
                                        ),
                                        (
                                          Icons.format_color_fill_outlined,
                                          'Fill',
                                        ),
                                        (Icons.colorize_outlined, 'Eyedropper'),
                                        (Icons.show_chart, 'Line'),
                                        (Icons.crop_square, 'Rectangle'),
                                        (Icons.circle_outlined, 'Ellipse'),
                                        (
                                          Icons.check_box_outline_blank,
                                          'Select',
                                        ),
                                        (Icons.pan_tool_outlined, 'Pan'),
                                      ])
                                        Padding(
                                          padding: const EdgeInsets.symmetric(
                                            vertical: 3,
                                          ),
                                          child: IconButton.filledTonal(
                                            tooltip: tool.$2,
                                            isSelected: editor.tool == tool.$2,
                                            style: IconButton.styleFrom(
                                              backgroundColor:
                                                  editor.tool == tool.$2
                                                  ? Theme.of(context)
                                                        .colorScheme
                                                        .primaryContainer
                                                  : Colors.transparent,
                                            ),
                                            onPressed:
                                                editor.busy || editor.drawing
                                                ? null
                                                : () => editor.setTool(tool.$2),
                                            icon: Icon(tool.$1),
                                          ),
                                        ),
                                      IconButton(
                                        tooltip: 'Selection options',
                                        onPressed: available
                                            ? () => showDocumentTools(
                                                context,
                                                editor,
                                                tab: 3,
                                              )
                                            : null,
                                        icon: const Icon(Icons.tune),
                                      ),
                                      const Padding(
                                        padding: EdgeInsets.only(bottom: 14),
                                        child: Icon(
                                          Icons.touch_app_outlined,
                                          size: 20,
                                        ),
                                      ),
                                    ],
                                  ),
                                ),
                              ],
                            );
                          },
                        ),
                      ),
                      const Divider(height: 1),
                      Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 12,
                          vertical: 6,
                        ),
                        child: Row(
                          children: [
                            Expanded(
                              child: Text(
                                editor.status,
                                overflow: TextOverflow.ellipsis,
                                style: Theme.of(context).textTheme.bodySmall,
                              ),
                            ),
                            if (!compact)
                              Text(
                                'Pinch to zoom • Two fingers to pan',
                                style: Theme.of(context).textTheme.bodySmall,
                              ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
      );
    },
  );
}

class Inspector extends StatelessWidget {
  final EditorController editor;
  const Inspector({super.key, required this.editor});
  static const colors = [
    ('Cream', 0xfff7e6b6),
    ('Teal', 0xff40c7bd),
    ('Rose', 0xffe76072),
    ('Gold', 0xfff0bc54),
    ('Leaf', 0xff8bbd66),
    ('Blue', 0xff6c9fe6),
    ('Lilac', 0xffaa91cd),
    ('Earth', 0xff845c52),
    ('White', 0xfffafafa),
    ('Ink', 0xff303541),
  ];
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Expanded(
        child: Material(
          color: Colors.transparent,
          clipBehavior: Clip.hardEdge,
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('Colors', style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 14),
                Row(
                  children: [
                    Container(
                      width: 30,
                      height: 30,
                      decoration: BoxDecoration(
                        color: Color(editor.color),
                        borderRadius: BorderRadius.circular(5),
                        border: Border.all(color: Colors.white24),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Flexible(
                      child: Text(
                        '#${(editor.color & 0xffffff).toRadixString(16).toUpperCase().padLeft(6, '0')}',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                Wrap(
                  children: colors
                      .map(
                        (entry) => Semantics(
                          label: '${entry.$1} color',
                          selected: editor.color == entry.$2,
                          button: true,
                          child: Tooltip(
                            message: entry.$1,
                            child: InkWell(
                              onTap: editor.drawing || editor.busy
                                  ? null
                                  : () => editor.setColor(entry.$2),
                              borderRadius: BorderRadius.circular(8),
                              child: Container(
                                width: 46,
                                height: 46,
                                padding: const EdgeInsets.all(5),
                                child: DecoratedBox(
                                  decoration: BoxDecoration(
                                    color: Color(entry.$2),
                                    borderRadius: BorderRadius.circular(5),
                                    border: Border.all(
                                      width: editor.color == entry.$2 ? 3 : 1,
                                      color: editor.color == entry.$2
                                          ? Theme.of(context)
                                                .colorScheme
                                                .primary
                                          : Colors.white24,
                                    ),
                                  ),
                                  child: editor.color == entry.$2
                                      ? Icon(
                                          Icons.check,
                                          size: 18,
                                          color:
                                              Color(entry.$2)
                                                      .computeLuminance() >
                                                  .4
                                              ? Colors.black
                                              : Colors.white,
                                        )
                                      : null,
                                ),
                              ),
                            ),
                          ),
                        ),
                      )
                      .toList(),
                ),
                const SizedBox(height: 18),
                const Divider(),
                const SizedBox(height: 10),
                OutlinedButton.icon(
                  onPressed: editor.busy || editor.drawing
                      ? null
                      : () async {
                          final entry = await editColor(context, editor.color);
                          if (entry != null) {
                            await editor.addNamedColor(entry.$1, entry.$2);
                          }
                        },
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Add color'),
                ),
                if (editor.namedColors.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    child: DropdownButton<int>(
                      isExpanded: true,
                      hint: const Text('Saved colors'),
                      value:
                          editor.namedColors.indexWhere(
                                (c) => c.$2 == editor.color,
                              ) <
                              0
                          ? null
                          : editor.namedColors.indexWhere(
                              (c) => c.$2 == editor.color,
                            ),
                      items: [
                        for (var i = 0; i < editor.namedColors.length; i++)
                          DropdownMenuItem(
                            value: i,
                            child: Text(
                              editor.namedColors[i].$1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                      ],
                      onChanged: editor.busy || editor.drawing
                          ? null
                          : (i) {
                              if (i != null) {
                                editor.setColor(editor.namedColors[i].$2);
                              }
                            },
                    ),
                  ),
                const Divider(),
                TextButton.icon(
                  onPressed: editor.busy || editor.drawing
                      ? null
                      : () => showDialog<void>(
                          context: context,
                          builder: (c) => StatefulBuilder(
                            builder: (c, update) => AlertDialog(
                              title: const Text('Brush and shapes'),
                              scrollable: true,
                              content: SizedBox(
                                width: 280,
                                child: Column(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    DropdownButtonFormField<int>(
                                      initialValue: editor.brushShape,
                                      decoration: const InputDecoration(
                                        labelText: 'Brush shape',
                                      ),
                                      items: [
                                        for (final e in [
                                          'Round',
                                          'Square',
                                          'Diamond',
                                          'Horizontal',
                                          'Vertical',
                                        ].asMap().entries)
                                          DropdownMenuItem(
                                            value: e.key,
                                            child: Text(e.value),
                                          ),
                                      ],
                                      onChanged: (n) async {
                                        if (n != null) {
                                          await editor.setBrushShape(n);
                                          if (c.mounted) update(() {});
                                        }
                                      },
                                    ),
                                    SwitchListTile(
                                      title: const Text('Stylus pressure'),
                                      subtitle: const Text(
                                        'Vary brush size with pressure. Off keeps crisp, constant pixel sizes.',
                                      ),
                                      value: editor.stylusPressure,
                                      onChanged: (v) {
                                        editor.setStylusPressure(v);
                                        if (c.mounted) update(() {});
                                      },
                                    ),
                                    SwitchListTile(
                                      title: const Text('Filled shapes'),
                                      value: editor.filled,
                                      onChanged: (v) {
                                        editor.setFilled(v);
                                        if (c.mounted) update(() {});
                                      },
                                    ),
                                  ],
                                ),
                              ),
                              actions: [
                                TextButton(
                                  onPressed: () => Navigator.pop(c),
                                  child: const Text('Done'),
                                ),
                              ],
                            ),
                          ),
                        ),
                  icon: const Icon(Icons.tune, size: 18),
                  label: const Text('Brush options'),
                ),
                TextButton.icon(
                  onPressed: editor.ready && !editor.busy && !editor.drawing
                      ? () => showDocumentTools(context, editor)
                      : null,
                  icon: const Icon(Icons.layers_outlined, size: 18),
                  label: const Text('Layers…'),
                ),
                Text('Layers', style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 8),
                for (final layer
                    in (editor.state['layers'] as List? ?? []).asMap().entries)
                  ListTile(
                    contentPadding: const EdgeInsets.symmetric(horizontal: 4),
                    dense: true,
                    minVerticalPadding: 12,
                    selected: editor.state['layer'] == layer.key,
                    selectedTileColor: Theme.of(context)
                        .colorScheme
                        .primaryContainer,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(6),
                    ),
                    leading: Icon(
                      layer.value['visible'] == true
                          ? Icons.layers_outlined
                          : Icons.visibility_off_outlined,
                      size: 19,
                    ),
                    title: Text(
                      layer.value['name'] as String,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontSize: 13),
                    ),
                    trailing: layer.value['locked'] == true
                        ? const Icon(Icons.lock_outline, size: 16)
                        : null,
                    onTap: editor.busy || editor.drawing
                        ? null
                        : () => editor.command(10, a: layer.key),
                  ),
              ],
            ),
          ),
        ),
      ),
      Padding(
        padding: const EdgeInsets.all(16),
        child: Material(
          key: const Key('preview-card'),
          color: Theme.of(context).colorScheme.surfaceContainer,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(10),
            side: BorderSide(color: Theme.of(context).dividerColor),
          ),
          clipBehavior: Clip.antiAlias,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 14, 12, 12),
                child: Text(
                  'Preview',
                  style: Theme.of(context).textTheme.titleSmall,
                ),
              ),
              Container(
                height: 144,
                margin: const EdgeInsets.fromLTRB(8, 0, 8, 8),
                color: Theme.of(context).colorScheme.surfaceContainerHighest,
                child: AnimatedBuilder(
                  animation: editor.canvasUpdates,
                  builder: (context, _) => editor.image == null
                      ? const Center(child: CircularProgressIndicator())
                      : Padding(
                          padding: const EdgeInsets.all(8),
                          child: RawImage(
                            image: editor.image,
                            fit: BoxFit.contain,
                            filterQuality: FilterQuality.none,
                          ),
                        ),
                ),
              ),
            ],
          ),
        ),
      ),
    ],
  );
}
