import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/theme/mpt_theme.dart';
import '../../shared/format.dart';
import '../../shared/title_bar.dart';
import 'mini_player_notifier.dart';
import 'timer_notifier.dart';

/// Compact always-on-top layout: subject, clock, and the two actions that
/// matter while a session is running.
class MiniPlayerView extends ConsumerWidget {
  const MiniPlayerView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      children: [
        MptTitleBar(
          isMini: true,
          onToggleMini: () => ref.read(miniPlayerProvider.notifier).toggle(),
        ),
        Container(height: 1, color: MptColors.line),
        const Expanded(child: _MiniBody()),
      ],
    );
  }
}

class _MiniBody extends ConsumerWidget {
  const _MiniBody();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final subject =
        ref.watch(timerProvider.select((s) => s.currentSubject?.title));
    final elapsed = ref.watch(timerProvider.select((s) => s.sessionElapsedMs));
    final running = ref.watch(timerProvider.select((s) => s.isRunning));
    final resting = ref.watch(timerProvider.select((s) => s.isResting));
    final busy = ref.watch(timerProvider.select((s) => s.busy));
    final notifier = ref.read(timerProvider.notifier);

    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 10, 12, 12),
      child: Row(
        children: [
          Expanded(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    if (running)
                      Container(
                        width: 4,
                        height: 4,
                        margin: const EdgeInsets.only(right: 6),
                        decoration: const BoxDecoration(
                          color: MptColors.accent,
                          shape: BoxShape.circle,
                        ),
                      ),
                    Flexible(
                      child: Text(
                        subject ?? 'No subject',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontFamily: MptFonts.ui,
                          fontSize: 11,
                          letterSpacing: 0.3,
                          color: MptColors.inkDim,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 2),
                Text(
                  formatClock(elapsed),
                  style: TextStyle(
                    fontFamily: MptFonts.mono,
                    fontSize: 34,
                    height: 1,
                    fontWeight: FontWeight.w300,
                    letterSpacing: -1.5,
                    color: running ? MptColors.ink : MptColors.inkDim,
                  ),
                ),
              ],
            ),
          ),
          _MiniAction(
            icon: resting || !running
                ? Icons.play_arrow_rounded
                : Icons.pause_rounded,
            tooltip: resting || !running ? 'Start' : 'Pause',
            onTap: busy
                ? null
                : (resting || !running ? notifier.start : notifier.pause),
          ),
          _MiniAction(
            icon: Icons.stop_rounded,
            tooltip: 'Stop',
            accent: true,
            onTap: busy || (!running && !resting) ? null : notifier.stop,
          ),
        ],
      ),
    );
  }
}

class _MiniAction extends StatelessWidget {
  const _MiniAction({
    required this.icon,
    required this.tooltip,
    this.onTap,
    this.accent = false,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback? onTap;
  final bool accent;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(2),
        hoverColor: accent ? MptColors.accentSoft : MptColors.line,
        child: SizedBox(
          width: 36,
          height: 36,
          child: Icon(
            icon,
            size: 19,
            color: onTap == null
                ? MptColors.inkFaint
                : accent
                    ? MptColors.accent
                    : MptColors.ink,
          ),
        ),
      ),
    );
  }
}
