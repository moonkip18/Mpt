import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';

import '../../core/theme/mpt_theme.dart';
import '../../data/daily_stats.dart';
import '../../data/models/subject_model.dart';
import '../../shared/format.dart';
import '../../shared/title_bar.dart';
import '../auth/auth_notifier.dart';
import 'mini_player_notifier.dart';
import 'mini_player_view.dart';
import 'offline_sync_notifier.dart';
import 'timer_notifier.dart';

class HomeScreen extends ConsumerStatefulWidget {
  const HomeScreen({super.key});

  @override
  ConsumerState<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends ConsumerState<HomeScreen> with WindowListener {
  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    Future.microtask(() {
      final sync = ref.read(offlineSyncNotifierProvider.notifier);
      if (ref.read(offlineSyncNotifierProvider).hasPending) {
        sync.syncNow();
      }
    });
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  /// Coming back to the window is the moment phone-side study is most likely to
  /// have happened, so pull the day's totals again instead of waiting for the
  /// next poll.
  @override
  void onWindowFocus() {
    ref.read(timerProvider.notifier).refreshStats();
  }

  @override
  Widget build(BuildContext context) {
    final isMini = ref.watch(miniPlayerProvider);

    return Scaffold(
      backgroundColor: MptColors.background,
      body: isMini ? const MiniPlayerView() : const _FullView(),
    );
  }
}

class _FullView extends ConsumerWidget {
  const _FullView();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final isMini = ref.watch(miniPlayerProvider);
    return Column(
      children: [
        MptTitleBar(
          isMini: isMini,
          onToggleMini: () => ref.read(miniPlayerProvider.notifier).toggle(),
        ),
        Container(height: 1, color: MptColors.line),
        const Expanded(
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Expanded(child: _StudyColumn()),
              VerticalDivider(width: 1, thickness: 1, color: MptColors.line),
              SizedBox(width: 316, child: SubjectPanel()),
            ],
          ),
        ),
      ],
    );
  }
}

class _StudyColumn extends ConsumerWidget {
  const _StudyColumn();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final running = ref.watch(timerProvider.select((s) => s.isRunning));
    final resting = ref.watch(timerProvider.select((s) => s.isResting));
    final totalMs = ref.watch(timerProvider.select((s) => s.displayTotalMs));

    return Padding(
      padding: const EdgeInsets.fromLTRB(34, 26, 34, 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _Header(totalMs: totalMs, running: running || resting),
          const Spacer(),
          const _ClockBlock(),
          const Spacer(),
          _SubjectStrip(enabled: !running && !resting),
          const SizedBox(height: 22),
          const _Controls(),
          const SizedBox(height: 14),
          const _NoticeLine(),
        ],
      ),
    );
  }
}

class _Header extends ConsumerWidget {
  const _Header({required this.totalMs, required this.running});

  final int totalMs;
  final bool running;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final date = DateTime.now();
    final restMs = ref.watch(timerProvider.select((s) => s.stats.restMs));
    final remote = ref.watch(timerProvider.select((s) => s.remoteActive));

    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('STUDY LOG', style: MptText.section),
            const SizedBox(height: 8),
            Text(
              formatDayLabel(date),
              style: const TextStyle(
                fontFamily: MptFonts.display,
                fontSize: 20,
                height: 1.1,
                color: MptColors.ink,
              ),
            ),
          ],
        ),
        const Spacer(),
        Column(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            const Text('TODAY', style: MptText.section),
            const SizedBox(height: 6),
            Text(
              formatHm(totalMs),
              style: const TextStyle(
                fontFamily: MptFonts.mono,
                fontSize: 26,
                height: 1,
                letterSpacing: -0.8,
                color: MptColors.ink,
              ),
            ),
            const SizedBox(height: 6),
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (restMs > 0) ...[
                  Text('rest ${formatHm(restMs)}', style: MptText.label),
                  const SizedBox(width: 10),
                ],
                if (remote)
                  const Text(
                    'PHONE LIVE',
                    style: TextStyle(
                      fontFamily: MptFonts.mono,
                      fontSize: 9,
                      letterSpacing: 1,
                      color: MptColors.accent,
                    ),
                  ),
              ],
            ),
          ],
        ),
      ],
    );
  }
}

class _ClockBlock extends ConsumerWidget {
  const _ClockBlock();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final subject =
        ref.watch(timerProvider.select((s) => s.currentSubject?.title));
    final elapsed = ref.watch(timerProvider.select((s) => s.sessionElapsedMs));
    final running = ref.watch(timerProvider.select((s) => s.isRunning));
    final resting = ref.watch(timerProvider.select((s) => s.isResting));

    final state = running
        ? 'STUDYING'
        : resting
            ? 'RESTING'
            : elapsed > 0
                ? 'PAUSED'
                : 'READY';

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            if (running)
              Container(
                width: 5,
                height: 5,
                margin: const EdgeInsets.only(right: 8),
                decoration: const BoxDecoration(
                  color: MptColors.accent,
                  shape: BoxShape.circle,
                ),
              ),
            Text(state, style: MptText.section),
          ],
        ),
        const SizedBox(height: 12),
        Text(
          subject ?? 'No subject',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(
            fontFamily: MptFonts.display,
            fontStyle: FontStyle.italic,
            fontSize: 27,
            height: 1.1,
            color: MptColors.ink,
          ),
        ),
        const SizedBox(height: 8),
        RepaintBoundary(
          child: Text(
            formatClock(elapsed),
            style: TextStyle(
              fontFamily: MptFonts.mono,
              fontSize: 92,
              height: 1,
              fontWeight: FontWeight.w300,
              letterSpacing: -4,
              color: running ? MptColors.ink : MptColors.inkDim,
            ),
          ),
        ),
      ],
    );
  }
}

class _SubjectStrip extends ConsumerWidget {
  const _SubjectStrip({required this.enabled});

  final bool enabled;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final subjects = ref.watch(timerProvider.select((s) => s.subjects));
    final currentId =
        ref.watch(timerProvider.select((s) => s.currentSubject?.id));

    if (subjects.isEmpty) {
      return const Text(
        'No subjects yet. Create one in the YPT app and it will appear here.',
        style: MptText.bodyDim,
      );
    }

    return SizedBox(
      height: 30,
      child: Row(
        children: [
          Expanded(
            child: ListView.separated(
              scrollDirection: Axis.horizontal,
              itemCount: subjects.length,
              separatorBuilder: (_, __) => const SizedBox(width: 8),
              itemBuilder: (context, index) {
                final s = subjects[index];
                final selected = s.id == currentId;
                return _SubjectChip(
                  subject: s,
                  selected: selected,
                  onTap: enabled
                      ? () => ref.read(timerProvider.notifier).selectSubject(s)
                      : null,
                );
              },
            ),
          ),
          if (subjects.length > 1) ...[
            const SizedBox(width: 8),
            _AllSubjectsButton(
              subjects: subjects,
              currentId: currentId,
              enabled: enabled,
              onSelected: (s) =>
                  ref.read(timerProvider.notifier).selectSubject(s),
            ),
          ],
        ],
      ),
    );
  }
}

/// The strip only has room for a handful of chips, so every subject is also one
/// click away in a full list — with its time for today next to it.
class _AllSubjectsButton extends ConsumerStatefulWidget {
  const _AllSubjectsButton({
    required this.subjects,
    required this.currentId,
    required this.enabled,
    required this.onSelected,
  });

  final List<SubjectModel> subjects;
  final int? currentId;
  final bool enabled;
  final ValueChanged<SubjectModel> onSelected;

  @override
  ConsumerState<_AllSubjectsButton> createState() => _AllSubjectsButtonState();
}

class _AllSubjectsButtonState extends ConsumerState<_AllSubjectsButton> {
  bool _hover = false;

  Future<void> _open() async {
    final box = context.findRenderObject() as RenderBox?;
    final overlay =
        Overlay.of(context).context.findRenderObject() as RenderBox?;
    if (box == null || overlay == null) return;

    final anchor = Rect.fromPoints(
      box.localToGlobal(Offset.zero, ancestor: overlay),
      box.localToGlobal(box.size.bottomRight(Offset.zero), ancestor: overlay),
    );
    final stats = ref.read(timerProvider).displayStats;

    final picked = await showMenu<int>(
      context: context,
      position: RelativeRect.fromRect(anchor, Offset.zero & overlay.size),
      items: [
        for (final s in widget.subjects)
          PopupMenuItem<int>(
            value: s.id,
            height: 34,
            padding: EdgeInsets.zero,
            child: _MenuRow(
              subject: s,
              selected: s.id == widget.currentId,
              todayMs: stats.msFor(title: s.title),
            ),
          ),
      ],
      constraints:
          const BoxConstraints(minWidth: 220, maxWidth: 340, maxHeight: 360),
    );

    if (picked == null || !mounted || !widget.enabled) return;
    for (final s in widget.subjects) {
      if (s.id == picked) {
        widget.onSelected(s);
        return;
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: 'All subjects',
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hover = true),
        onExit: (_) => setState(() => _hover = false),
        child: GestureDetector(
          onTap: _open,
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 120),
            width: 30,
            height: 30,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: _hover ? MptColors.surface : Colors.transparent,
              border: Border.all(
                  color: _hover ? MptColors.lineStrong : MptColors.line),
              borderRadius: BorderRadius.circular(2),
            ),
            child: Icon(
              Icons.chevron_right,
              size: 16,
              color: _hover ? MptColors.ink : MptColors.inkDim,
            ),
          ),
        ),
      ),
    );
  }
}

class _MenuRow extends StatelessWidget {
  const _MenuRow(
      {required this.subject, required this.selected, required this.todayMs});

  final SubjectModel subject;
  final bool selected;
  final int todayMs;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12),
      child: Row(
        children: [
          Container(
            width: 6,
            height: 6,
            decoration:
                BoxDecoration(color: subject.color, shape: BoxShape.circle),
          ),
          const SizedBox(width: 9),
          Expanded(
            child: Text(
              subject.title,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontFamily: MptFonts.ui,
                fontSize: 12.5,
                color: selected ? MptColors.ink : MptColors.inkDim,
                fontWeight: selected ? FontWeight.w600 : FontWeight.w400,
              ),
            ),
          ),
          if (todayMs > 0) ...[
            const SizedBox(width: 14),
            Text(
              formatHm(todayMs),
              style: MptText.label.copyWith(color: MptColors.inkFaint),
            ),
          ],
        ],
      ),
    );
  }
}

class _SubjectChip extends StatefulWidget {
  const _SubjectChip(
      {required this.subject, required this.selected, this.onTap});

  final SubjectModel subject;
  final bool selected;
  final VoidCallback? onTap;

  @override
  State<_SubjectChip> createState() => _SubjectChipState();
}

class _SubjectChipState extends State<_SubjectChip> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final enabled = widget.onTap != null;
    final bg = widget.selected
        ? MptColors.surfaceRaised
        : _hover && enabled
            ? MptColors.surface
            : Colors.transparent;

    return MouseRegion(
      cursor: enabled ? SystemMouseCursors.click : SystemMouseCursors.basic,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 120),
          padding: const EdgeInsets.symmetric(horizontal: 11),
          decoration: BoxDecoration(
            color: bg,
            border: Border.all(
              color: widget.selected ? MptColors.lineStrong : MptColors.line,
            ),
            borderRadius: BorderRadius.circular(2),
          ),
          child: Row(
            children: [
              Container(
                width: 6,
                height: 6,
                decoration: BoxDecoration(
                  color: widget.subject.color,
                  shape: BoxShape.circle,
                ),
              ),
              const SizedBox(width: 7),
              Text(
                widget.subject.title,
                style: TextStyle(
                  fontFamily: MptFonts.ui,
                  fontSize: 12,
                  color: widget.selected ? MptColors.ink : MptColors.inkDim,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Controls extends ConsumerWidget {
  const _Controls();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(timerProvider);
    final notifier = ref.read(timerProvider.notifier);
    final hasSubject = state.currentSubject != null;

    if (!state.isRunning && !state.isResting) {
      return Row(
        children: [
          _Button(
            label: state.sessionElapsedMs > 0 ? 'Resume' : 'Start',
            primary: true,
            enabled: hasSubject && !state.busy,
            onTap: notifier.start,
          ),
          if (state.sessionElapsedMs > 0) ...[
            const SizedBox(width: 10),
            _Button(
              label: 'Save & finish',
              enabled: !state.busy,
              onTap: notifier.stop,
            ),
          ],
        ],
      );
    }

    return Row(
      children: [
        _Button(
          label: state.isResting ? 'Resume' : 'Pause',
          primary: !state.isResting,
          enabled: !state.busy,
          onTap: state.isResting ? notifier.start : notifier.pause,
        ),
        const SizedBox(width: 10),
        _Button(
          label: 'Stop',
          accent: true,
          enabled: !state.busy,
          onTap: notifier.stop,
        ),
        const SizedBox(width: 14),
        if (state.busy)
          const SizedBox(
            width: 12,
            height: 12,
            child: CircularProgressIndicator(
                strokeWidth: 1.2, color: MptColors.inkFaint),
          ),
      ],
    );
  }
}

class _Button extends StatefulWidget {
  const _Button({
    required this.label,
    required this.onTap,
    this.primary = false,
    this.accent = false,
    this.enabled = true,
  });

  final String label;
  final VoidCallback onTap;
  final bool primary;
  final bool accent;
  final bool enabled;

  @override
  State<_Button> createState() => _ButtonState();
}

class _ButtonState extends State<_Button> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    Color bg;
    Color fg;
    Border? border;

    if (!widget.enabled) {
      bg = MptColors.surface;
      fg = MptColors.inkFaint;
    } else if (widget.primary) {
      bg = _hover ? Colors.white : MptColors.synapse;
      fg = MptColors.background;
    } else if (widget.accent) {
      bg = _hover ? MptColors.accent : Colors.transparent;
      fg = _hover ? MptColors.ink : MptColors.accent;
      border = Border.all(color: MptColors.accentSoft);
    } else {
      bg = _hover ? MptColors.surfaceRaised : Colors.transparent;
      fg = MptColors.ink;
      border = Border.all(color: MptColors.lineStrong);
    }

    return MouseRegion(
      cursor:
          widget.enabled ? SystemMouseCursors.click : SystemMouseCursors.basic,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.enabled ? widget.onTap : null,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 120),
          height: 42,
          padding: const EdgeInsets.symmetric(horizontal: 26),
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: bg,
            border: border,
            borderRadius: BorderRadius.circular(2),
          ),
          child: Text(
            widget.label,
            style: TextStyle(
              fontFamily: MptFonts.ui,
              fontSize: 13,
              fontWeight: FontWeight.w600,
              letterSpacing: 0.2,
              color: fg,
            ),
          ),
        ),
      ),
    );
  }
}

class _NoticeLine extends ConsumerStatefulWidget {
  const _NoticeLine();

  @override
  ConsumerState<_NoticeLine> createState() => _NoticeLineState();
}

class _NoticeLineState extends ConsumerState<_NoticeLine> {
  @override
  Widget build(BuildContext context) {
    final notice = ref.watch(timerProvider.select((s) => s.notice));
    if (notice == null) {
      return const SizedBox(height: 16);
    }
    return Row(
      children: [
        const Icon(Icons.info_outline_rounded,
            size: 13, color: MptColors.inkFaint),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            notice,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: MptText.bodyDim,
          ),
        ),
        GestureDetector(
          onTap: () => ref.read(timerProvider.notifier).clearNotice(),
          child: const Icon(Icons.close_rounded,
              size: 13, color: MptColors.inkFaint),
        ),
      ],
    );
  }
}

class SubjectPanel extends ConsumerWidget {
  const SubjectPanel({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final stats = ref.watch(timerProvider.select((s) => s.displayStats));
    final currentId =
        ref.watch(timerProvider.select((s) => s.currentSubject?.id));
    final idle =
        ref.watch(timerProvider.select((s) => !s.isRunning && !s.isResting));
    final user = ref.watch(authStateProvider).user;
    final pending = ref.watch(offlineSyncNotifierProvider);

    final rows = stats.subjects;
    final maxMs = rows.isEmpty ? 0 : rows.first.ms;

    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 26, 20, 18),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Text('TODAY BY SUBJECT', style: MptText.section),
              const Spacer(),
              Text(formatHmCompact(stats.totalMs), style: MptText.label),
            ],
          ),
          const SizedBox(height: 18),
          Expanded(
            child: rows.isEmpty
                ? const Align(
                    alignment: Alignment.topLeft,
                    child: Text(
                      'Nothing logged yet today.\nPress start and the hours appear here.',
                      style: MptText.bodyDim,
                    ),
                  )
                : ListView.builder(
                    padding: EdgeInsets.zero,
                    itemCount: rows.length,
                    itemBuilder: (context, index) {
                      final row = rows[index];
                      return _StatRow(
                        stat: row,
                        maxMs: maxMs,
                        isCurrent: idle && row.id == currentId,
                        onTap: idle
                            ? () {
                                SubjectModel? target;
                                for (final s
                                    in ref.read(timerProvider).subjects) {
                                  if (s.id == row.id) {
                                    target = s;
                                    break;
                                  }
                                }
                                if (target != null) {
                                  ref
                                      .read(timerProvider.notifier)
                                      .selectSubject(target);
                                }
                              }
                            : null,
                      );
                    },
                  ),
          ),
          if (pending.pendingCount > 0) ...[
            Container(height: 1, color: MptColors.line),
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: Text(
                    '${pending.pendingCount} session${pending.pendingCount == 1 ? '' : 's'} offline',
                    style: MptText.label.copyWith(color: MptColors.accent),
                  ),
                ),
                GestureDetector(
                  onTap: pending.isSyncing
                      ? null
                      : () => ref
                          .read(offlineSyncNotifierProvider.notifier)
                          .syncNow(),
                  child: Text(
                    pending.isSyncing ? 'syncing…' : 'sync now',
                    style: MptText.label.copyWith(color: MptColors.ink),
                  ),
                ),
              ],
            ),
          ],
          const SizedBox(height: 14),
          Container(height: 1, color: MptColors.line),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      user?.name ?? '',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: MptText.body,
                    ),
                    const SizedBox(height: 2),
                    const Text('connected to YPT', style: MptText.label),
                  ],
                ),
              ),
              Tooltip(
                message: 'Sign out',
                child: InkWell(
                  onTap: () async {
                    await ref.read(miniPlayerProvider.notifier).exit();
                    await ref.read(authStateProvider.notifier).logout();
                    // Drop the session state so a different account never
                    // inherits the previous subject or totals.
                    ref.invalidate(timerProvider);
                  },
                  child: const Padding(
                    padding: EdgeInsets.all(6),
                    child: Icon(Icons.logout_rounded,
                        size: 15, color: MptColors.inkFaint),
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _StatRow extends StatefulWidget {
  const _StatRow({
    required this.stat,
    required this.maxMs,
    required this.isCurrent,
    this.onTap,
  });

  final SubjectStat stat;
  final int maxMs;
  final bool isCurrent;
  final VoidCallback? onTap;

  @override
  State<_StatRow> createState() => _StatRowState();
}

class _StatRowState extends State<_StatRow> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final stat = widget.stat;
    final fraction =
        widget.maxMs <= 0 ? 0.0 : (stat.ms / widget.maxMs).clamp(0.0, 1.0);

    return MouseRegion(
      cursor: widget.onTap != null
          ? SystemMouseCursors.click
          : SystemMouseCursors.basic,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(vertical: 9, horizontal: 8),
          color: _hover ? MptColors.surface : Colors.transparent,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Container(
                    width: 2,
                    height: 12,
                    color: Color(stat.colorInt),
                  ),
                  const SizedBox(width: 9),
                  Expanded(
                    child: Text(
                      stat.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontFamily: MptFonts.ui,
                        fontSize: 12.5,
                        color:
                            widget.isCurrent ? MptColors.ink : MptColors.inkDim,
                      ),
                    ),
                  ),
                  Text(
                    formatHm(stat.ms),
                    style: const TextStyle(
                      fontFamily: MptFonts.mono,
                      fontSize: 11.5,
                      color: MptColors.ink,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 7),
              LayoutBuilder(
                builder: (context, constraints) {
                  return Stack(
                    children: [
                      Container(height: 2, color: MptColors.line),
                      TweenAnimationBuilder<double>(
                        tween: Tween(begin: 0, end: fraction),
                        duration: const Duration(milliseconds: 260),
                        curve: Curves.easeOut,
                        builder: (context, value, _) => Container(
                          width: constraints.maxWidth * value,
                          height: 2,
                          color: Color(stat.colorInt),
                        ),
                      ),
                    ],
                  );
                },
              ),
            ],
          ),
        ),
      ),
    );
  }
}
