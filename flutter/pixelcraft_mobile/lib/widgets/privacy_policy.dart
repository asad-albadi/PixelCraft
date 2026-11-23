import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Bundled policy stays available offline and does not touch the editor session.
Future<void> showPrivacyPolicy(BuildContext context) async {
  final policy = await rootBundle.loadString('assets/legal/privacy-policy.md');
  if (!context.mounted) return;
  await showDialog<void>(
    context: context,
    builder: (context) => Dialog(
      insetPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 24),
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxWidth: 640,
          maxHeight: MediaQuery.sizeOf(context).height * .85,
        ),
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      'Privacy policy',
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                  ),
                  IconButton(
                    tooltip: 'Close privacy policy',
                    constraints: const BoxConstraints(
                      minWidth: 48,
                      minHeight: 48,
                    ),
                    onPressed: () => Navigator.pop(context),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
              const Divider(),
              Flexible(
                child: SingleChildScrollView(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      for (final block in policy.trim().split('\n\n'))
                        if (!block.startsWith('# '))
                          Padding(
                            padding: const EdgeInsets.only(bottom: 16),
                            child: block.startsWith('## ')
                                ? Semantics(
                                    header: true,
                                    child: Text(
                                      block.substring(3),
                                      style: Theme.of(context)
                                          .textTheme
                                          .titleMedium,
                                    ),
                                  )
                                : SelectableText(block.replaceAll('**', '')),
                          ),
                    ],
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
