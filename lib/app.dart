import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';

import 'core/theme/mpt_theme.dart';
import 'features/auth/auth_notifier.dart';
import 'features/auth/login_screen.dart';
import 'features/timer/home_screen.dart';
import 'features/timer/timer_notifier.dart';

class MptApp extends ConsumerStatefulWidget {
  const MptApp({super.key, required this.container});

  /// Needed by the close handler: at that point the widget tree is already on
  /// its way out, but the session still has to be stopped and synced.
  final ProviderContainer container;

  @override
  ConsumerState<MptApp> createState() => _MptAppState();
}

class _MptAppState extends ConsumerState<MptApp> with WindowListener {
  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  /// YPT keeps a session open until an explicit stop, so closing the window
  /// mid-study would leave "studying" running forever on the account. Stop
  /// first — the stop is queued offline if it cannot reach the server — then
  /// let the window go.
  @override
  void onWindowClose() async {
    try {
      final container = widget.container;
      if (container.exists(timerProvider)) {
        final notifier = container.read(timerProvider.notifier);
        if (container.read(timerProvider).hasSession) {
          await notifier
              .stop()
              .timeout(const Duration(seconds: 8), onTimeout: () {});
        }
      }
    } catch (_) {
      // Never block the exit on a network problem.
    }
    try {
      await windowManager.destroy();
    } catch (_) {}
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Mpt',
      debugShowCheckedModeBanner: false,
      theme: MptTheme.build(),
      home: const _AuthGate(),
    );
  }
}

/// Single source of routing: authenticated or not. No router package needed for
/// exactly two destinations.
class _AuthGate extends ConsumerStatefulWidget {
  const _AuthGate();

  @override
  ConsumerState<_AuthGate> createState() => _AuthGateState();
}

class _AuthGateState extends ConsumerState<_AuthGate> {
  @override
  void initState() {
    super.initState();
    Future.microtask(
        () => ref.read(authStateProvider.notifier).checkAuthStatus());
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authStateProvider);

    if (auth.user != null) {
      return const HomeScreen();
    }
    if (auth.isLoading) {
      return const _BootScreen();
    }
    return const LoginScreen();
  }
}

class _BootScreen extends StatelessWidget {
  const _BootScreen();

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      backgroundColor: MptColors.background,
      body: Center(
        child: SizedBox(
          width: 16,
          height: 16,
          child: CircularProgressIndicator(
            strokeWidth: 1.5,
            color: MptColors.inkFaint,
          ),
        ),
      ),
    );
  }
}
