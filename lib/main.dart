import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:window_manager/window_manager.dart';

import 'app.dart';
import 'core/config/env_config.dart';
import 'core/theme/mpt_theme.dart';
import 'features/settings/settings_notifier.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  await EnvConfig.init();
  final prefs = await SharedPreferences.getInstance();

  await windowManager.ensureInitialized();
  const options = WindowOptions(
    size: Size(1180, 760),
    minimumSize: Size(900, 560),
    center: true,
    backgroundColor: MptColors.background,
    titleBarStyle: TitleBarStyle.hidden,
    title: 'Mpt',
  );

  windowManager.waitUntilReadyToShow(options, () async {
    await windowManager.show();
    await windowManager.focus();
    // Closing must run the session stop first, so intercept it.
    await windowManager.setPreventClose(true);
    if (!Platform.isWindows) {
      // Windows keeps the native resize border with TitleBarStyle.hidden.
      await windowManager.setAsFrameless();
    }
  });

  final container = ProviderContainer(
    overrides: [sharedPreferencesProvider.overrideWithValue(prefs)],
  );

  runApp(
    UncontrolledProviderScope(
      container: container,
      child: MptApp(container: container),
    ),
  );
}
