import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';

/// The mini player is the same window, shrunk and pinned on top, so there is no
/// second isolate, no IPC and no extra render target.
class MiniPlayerNotifier extends StateNotifier<bool> {
  MiniPlayerNotifier({bool initial = false}) : super(initial);

  static const compactSize = Size(360, 148);
  static const fullSize = Size(1180, 760);
  static const compactMinimum = Size(340, 140);
  static const fullMinimum = Size(900, 560);

  Size _restoreSize = fullSize;

  Future<void> toggle() async {
    final next = !state;
    try {
      if (next) {
        final current = await windowManager.getSize();
        if (current.width > 400) _restoreSize = current;
        state = true;
        // The full window refuses to shrink below 900px, so the minimum has to
        // move before the size does.
        await windowManager.setMinimumSize(compactMinimum);
        await windowManager.setSize(compactSize);
        await windowManager.setAlwaysOnTop(true);
        await windowManager.setResizable(false);
      } else {
        state = false;
        await windowManager.setAlwaysOnTop(false);
        await windowManager.setResizable(true);
        await windowManager.setMinimumSize(fullMinimum);
        await windowManager.setSize(_restoreSize);
        await windowManager.center();
      }
    } catch (_) {
      state = next;
    }
  }

  Future<void> exit() async {
    if (!state) return;
    await toggle();
  }
}

final miniPlayerProvider = StateNotifierProvider<MiniPlayerNotifier, bool>(
    (ref) => MiniPlayerNotifier());
