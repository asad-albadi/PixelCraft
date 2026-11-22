import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../editor_controller.dart';

class EditorKeyboard extends StatelessWidget {
  final EditorController editor;
  final Widget child;
  const EditorKeyboard({super.key, required this.editor, required this.child});
  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {
      const SingleActivator(LogicalKeyboardKey.keyZ, control: true): () {
        if (editor.canUndo) editor.command(3);
      },
      const SingleActivator(
        LogicalKeyboardKey.keyZ,
        control: true,
        shift: true,
      ): () {
        if (editor.canRedo) editor.command(4);
      },
      const SingleActivator(LogicalKeyboardKey.keyY, control: true): () {
        if (editor.canRedo) editor.command(4);
      },
      const SingleActivator(LogicalKeyboardKey.keyS, control: true): () =>
          editor.save(),
      const SingleActivator(
        LogicalKeyboardKey.keyF,
        control: true,
        shift: true,
      ): () {
        if (editor.zen) {
          editor.exitZen();
        } else {
          editor.setZen(true);
        }
      },
      const SingleActivator(LogicalKeyboardKey.escape): () {
        if (editor.zen) {
          editor.exitZen();
        } else if (editor.drawing) {
          editor.cancel();
        } else if (editor.selection != null) {
          editor.selectionAction('deselect');
        }
      },
      const SingleActivator(LogicalKeyboardKey.keyB): () =>
          editor.setTool('Draw'),
      const SingleActivator(LogicalKeyboardKey.keyE): () =>
          editor.setTool('Erase'),
      const SingleActivator(LogicalKeyboardKey.keyG): () =>
          editor.setTool('Fill'),
      const SingleActivator(LogicalKeyboardKey.keyI): () =>
          editor.setTool('Eyedropper'),
      const SingleActivator(LogicalKeyboardKey.keyM): () =>
          editor.setTool('Select'),
    },
    child: Focus(autofocus: true, child: child),
  );
}
