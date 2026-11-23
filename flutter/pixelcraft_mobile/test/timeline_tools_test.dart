import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/editor_controller.dart';
import 'package:pixelcraft_mobile/main.dart';
import 'package:pixelcraft_mobile/widgets/document_tools.dart';

class TargetEditor extends EditorController {
  final mutations = <Map<String, dynamic>>[];
  @override
  bool get ready => true;
  @override
  Future<void> command(int op, {int a = 0, int b = 0, Uint8List? input}) async {
    if (op == 10) state['layer'] = a;
    notifyListeners();
  }

  @override
  Future<void> edits(List<Map<String, dynamic>> commands) async {
    mutations.addAll(commands);
  }

  void setBusy(bool value) {
    busy = value;
    notifyListeners();
  }
}

EditorController fixture() => EditorController()
  ..state = {
    'width': 48,
    'height': 48,
    'frames': 2,
    'frame': 0,
    'durations': [100, 80],
    'frame_names': ['idle_01', 'walk_01'],
    'layer': 0,
    'layers': [
      {
        'name': 'Garden',
        'visible': true,
        'locked': false,
        'group': false,
        'opacity': 255,
      },
      {
        'name': 'Fireflies',
        'visible': true,
        'locked': false,
        'group': false,
        'opacity': 255,
      },
    ],
    'tags': [],
    'undo': 0,
    'redo': 0,
    'dirty': false,
  };
void main() {
  testWidgets(
    'Layer access presents visible rows and inline controls instead of a selector',
    (tester) async {
      final editor = fixture();
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) => Scaffold(
              body: TextButton(
                onPressed: () => showDocumentTools(context, editor),
                child: const Text('Layers'),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Layers'));
      await tester.pumpAndSettle();
      expect(find.text('Garden'), findsOneWidget);
      expect(find.text('Fireflies'), findsOneWidget);
      expect(find.byTooltip('Hide Garden'), findsOneWidget);
      expect(find.byType(TabBar), findsNothing);
      expect(find.byType(DropdownButton<int>), findsNothing);
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
    },
  );
  testWidgets(
    'Tablet timeline exposes layers beside frame cells and selection actions near canvas',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final editor = fixture();
      editor.state['selection'] = [1, 1, 4, 4];
      editor.state['mode'] = 'Indexed';
      await tester.pumpWidget(PixelCraftApp(controller: editor));
      await tester.pump();
      expect(find.byTooltip('Hide Garden'), findsOneWidget);
      expect(find.byTooltip('Layer options for Fireflies'), findsOneWidget);
      expect(find.byTooltip('Copy selection'), findsOneWidget);
      expect(find.text('48 × 48 px  /  Indexed'), findsOneWidget);
      expect(find.byTooltip('Deselect'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
    },
  );
  testWidgets(
    'Eye toggles preserve drawing target and busy properties retain their input',
    (tester) async {
      final editor = TargetEditor()..state = fixture().state;
      editor.state['layer'] = 1;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: LayerRow(editor: editor, index: 0)),
        ),
      );
      await tester.tap(find.byTooltip('Hide Garden'));
      await tester.pump();
      expect(editor.state['layer'], 1);
      expect(editor.mutations.single, {
        'op': 'set-layer',
        'visible': false,
        'layer': 0,
      });
      await tester.tap(find.byTooltip('Layer options for Garden'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Properties…'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'Renamed garden');
      editor.setBusy(true);
      await tester.pump();
      final apply = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, 'Apply'),
      );
      expect(apply.onPressed, isNull);
      expect(find.text('Renamed garden'), findsOneWidget);
      editor.setBusy(false);
      await tester.pump();
      await tester.tap(find.text('Apply'));
      await tester.pumpAndSettle();
      expect(editor.mutations.last['name'], 'Renamed garden');
      await tester.pumpWidget(const SizedBox());
      editor.dispose();
    },
  );
}
