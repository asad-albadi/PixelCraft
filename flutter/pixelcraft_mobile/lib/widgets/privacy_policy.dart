import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Android opens the fixed policy URL in the user's browser; no policy is bundled.
Future<void> openPrivacyPolicy(BuildContext context) async {
  try {
    await const MethodChannel('pixelcraft/links')
        .invokeMethod<void>('privacyPolicy');
  } on PlatformException {
    if (!context.mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Could not open the browser. Please try again.'),
      ),
    );
  }
}
