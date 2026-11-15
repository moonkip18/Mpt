// Visual preview: renders the real screens with the system font files loaded so
// the layout can be reviewed as PNGs. Run with:
//   flutter test tool/preview_test.dart --update-goldens
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mpt/core/theme/mpt_theme.dart';
import 'package:mpt/data/repositories/offline_sync_repository.dart';
import 'package:mpt/features/auth/login_screen.dart';
import 'package:mpt/features/settings/settings_notifier.dart';
import 'package:mpt/features/timer/home_screen.dart';
import 'package:mpt/features/timer/mini_player_notifier.dart';
import 'package:mpt/features/timer/timer_notifier.dart';
import 'package:mpt/features/timer/timer_session_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../test/fakes.dart';

Future<void> _loadSystemFonts() async {
  Future<void> load(String family, List<String> files) async {
    final loader = FontLoader(family);
    for (final file in files) {
      final path = 'C:\\Windows\\Fonts\\$file';
      if (!File(path).existsSync()) continue;
      final bytes = File(path).readAsBytesSync();
      loader.addFont(Future<ByteData>.value(ByteData.view(bytes.buffer)));
    }
    await loader.load();
  }

  await load('Segoe UI', ['segoeui.ttf', 'segoeuib.ttf', 'segoeuisl.ttf']);
  await load('Georgia', ['georgia.ttf', 'georgiai.ttf', 'georgiab.ttf']);
  await load('Consolas', ['consola.ttf', 'consolab.ttf']);
}

class _Clock {
  _Clock(this.value);
  DateTime value;
  DateTime now() => value;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late SharedPreferences prefs;

  setUpAll(() async {
    await _loadSystemFonts();
  });

  setUp(() async {
    // ignore: invalid_use_of_visible_for_testing_member
    SharedPreferences.setMockInitialValues(<String, Object>{});
    prefs = await SharedPreferences.getInstance();
  });

  void sizeTo(WidgetTester tester, Size size) {
    tester.view.physicalSize = size;
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
  }

  Future<TimerNotifier> pumpHome(
    WidgetTester tester, {
    required Size size,
    bool running = false,
    bool mini = false,
  }) async {
    sizeTo(tester, size);
    final clock = _Clock(DateTime(2026, 11, 15, 9));
    final timerRepo = FakeTimerRepository();
    final notifier = TimerNotifier(
      timerRepository: timerRepo,
      subjectRepository: FakeSubjectRepository(),
      offlineSync:
          OfflineSyncRepository(timerRepository: timerRepo, prefs: prefs),
      sessionStore: TimerSessionStore(prefs: prefs),
      dayResetHour: 5,
      language: 'en',
      timezone: 'Asia/Tehran',
      clock: clock.now,
    );

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          sharedPreferencesProvider.overrideWithValue(prefs),
          timerProvider.overrideWith((ref) => notifier),
          miniPlayerProvider
              .overrideWith((ref) => MiniPlayerNotifier(initial: mini)),
        ],
        child: MaterialApp(theme: MptTheme.build(), home: const HomeScreen()),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 60));

    if (running) {
      await notifier.start();
      clock.value =
          clock.value.add(const Duration(hours: 1, minutes: 12, seconds: 40));
      await tester.pump(const Duration(seconds: 1));
    }
    await tester.pump(const Duration(milliseconds: 300));
    return notifier;
  }

  testWidgets('login preview', (tester) async {
    sizeTo(tester, const Size(1180, 760));
    await tester.pumpWidget(
      ProviderScope(
        overrides: [sharedPreferencesProvider.overrideWithValue(prefs)],
        child: MaterialApp(theme: MptTheme.build(), home: const LoginScreen()),
      ),
    );
    await tester.pump();

    await expectLater(
      find.byType(LoginScreen),
      matchesGoldenFile('../docs/screenshots/login.png'),
    );
  });

  testWidgets('home idle preview', (tester) async {
    await pumpHome(tester, size: const Size(1180, 760));

    await expectLater(
      find.byType(HomeScreen),
      matchesGoldenFile('../docs/screenshots/home_idle.png'),
    );
  });

  testWidgets('home running preview', (tester) async {
    await pumpHome(tester, size: const Size(1180, 760), running: true);

    await expectLater(
      find.byType(HomeScreen),
      matchesGoldenFile('../docs/screenshots/home_running.png'),
    );
  });

  testWidgets('mini player preview', (tester) async {
    await pumpHome(
      tester,
      size: const Size(360, 148),
      running: true,
      mini: true,
    );

    await expectLater(
      find.byType(HomeScreen),
      matchesGoldenFile('../docs/screenshots/mini.png'),
    );
  });
}
