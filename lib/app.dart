import 'dart:io';

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

  bool _closing = false;

  /// YPT keeps a session open until an explicit stop, so closing mid-study has
  /// to stop it first — but an unreachable server must never make the window
  /// look frozen. Every step here is bounded, and the process exits no matter
  /// what happened.
  @override
  void onWindowClose() {
    if (_closing) return;
    _closing = true;

    final container = widget.container;
    Future<void>(() async {
      try {
        if (container.exists(timerProvider) &&
            container.read(timerProvider).hasSession) {
          await container
              .read(timerProvider.notifier)
              .stop()
              .timeout(const Duration(milliseconds: 1500), onTimeout: () {});
        }
      } catch (_) {
        // The session snapshot stays on disk, so nothing is lost.
      }
      try {
        await windowManager.destroy().timeout(
              const Duration(milliseconds: 600),
              onTimeout: () {},
            );
      } catch (_) {}
      exit(0);
    });
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
