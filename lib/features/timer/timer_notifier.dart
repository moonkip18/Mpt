import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/constants/api_constants.dart';
import '../../core/utils/study_date_helper.dart';
import '../../data/daily_stats.dart';
import '../../data/models/subject_model.dart';
import '../../data/repositories/offline_sync_repository.dart';
import '../../data/repositories/subject_repository.dart';
import '../../data/repositories/timer_repository.dart';
import '../settings/settings_notifier.dart';
import 'offline_sync_notifier.dart';
import 'timer_session_store.dart';

final timerRepositoryProvider = Provider<TimerRepository>((ref) {
  return TimerRepository();
});

final subjectRepositoryProvider = Provider<SubjectRepository>((ref) {
  return SubjectRepository();
});

final timerSessionStoreProvider = Provider<TimerSessionStore>((ref) {
  return TimerSessionStore(prefs: ref.watch(sharedPreferencesProvider));
});

class TimerState {
  final List<SubjectModel> subjects;
  final SubjectModel? currentSubject;

  /// Server truth for the current study day.
  final DailyStats stats;

  final bool isRunning;
  final bool isResting;

  /// Instant the current running segment began; null while resting/idle.
  final DateTime? segmentStartAt;
  final int accumulatedMs;
  final int sessionElapsedMs;

  final int sessionRestMs;
  final DateTime? restStartAt;

  /// Elapsed milliseconds already stored on YPT for this session.
  final int lastSyncedMs;

  /// Session elapsed when the current segment began. YPT receives the segment
  /// duration, not the whole session, so a paused-and-resumed session is not
  /// recorded twice.
  final int segmentBaseMs;

  final String lastStudyDate;
  final bool busy;
  final String? notice;

  /// Another device has an open session, so totals may move without us.
  final bool remoteActive;

  const TimerState({
    this.subjects = const [],
    this.currentSubject,
    this.stats = DailyStats.empty,
    this.isRunning = false,
    this.isResting = false,
    this.segmentStartAt,
    this.accumulatedMs = 0,
    this.sessionElapsedMs = 0,
    this.sessionRestMs = 0,
    this.restStartAt,
    this.lastSyncedMs = 0,
    this.segmentBaseMs = 0,
    this.lastStudyDate = '',
    this.busy = false,
    this.notice,
    this.remoteActive = false,
  });

  bool get hasSession => isRunning || isResting || sessionElapsedMs > 0;

  /// Time in this session that YPT does not know about yet.
  int get liveDeltaMs {
    final delta = sessionElapsedMs - lastSyncedMs;
    return delta > 0 ? delta : 0;
  }

  /// Today's total as the user should see it right now.
  int get displayTotalMs => stats.totalMs + liveDeltaMs;

  /// Per-subject totals with the unsynced remainder folded into the current row.
  DailyStats get displayStats => stats.withLiveDelta(
        subjectId: currentSubject?.id,
        subjectTitle: currentSubject?.title,
        liveMs: liveDeltaMs,
      );

  TimerState copyWith({
    List<SubjectModel>? subjects,
    SubjectModel? currentSubject,
    DailyStats? stats,
    bool? isRunning,
    bool? isResting,
    DateTime? segmentStartAt,
    bool clearSegmentStart = false,
    int? accumulatedMs,
    int? sessionElapsedMs,
    int? sessionRestMs,
    DateTime? restStartAt,
    bool clearRestStart = false,
    int? lastSyncedMs,
    int? segmentBaseMs,
    String? lastStudyDate,
    bool? busy,
    String? notice,
    bool clearNotice = false,
    bool? remoteActive,
  }) {
    return TimerState(
      subjects: subjects ?? this.subjects,
      currentSubject: currentSubject ?? this.currentSubject,
      stats: stats ?? this.stats,
      isRunning: isRunning ?? this.isRunning,
      isResting: isResting ?? this.isResting,
      segmentStartAt:
          clearSegmentStart ? null : (segmentStartAt ?? this.segmentStartAt),
      accumulatedMs: accumulatedMs ?? this.accumulatedMs,
      sessionElapsedMs: sessionElapsedMs ?? this.sessionElapsedMs,
      sessionRestMs: sessionRestMs ?? this.sessionRestMs,
      restStartAt: clearRestStart ? null : (restStartAt ?? this.restStartAt),
      lastSyncedMs: lastSyncedMs ?? this.lastSyncedMs,
      segmentBaseMs: segmentBaseMs ?? this.segmentBaseMs,
      lastStudyDate: lastStudyDate ?? this.lastStudyDate,
      busy: busy ?? this.busy,
      notice: notice ?? (clearNotice ? null : this.notice),
      remoteActive: remoteActive ?? this.remoteActive,
    );
  }
}

class TimerNotifier extends StateNotifier<TimerState> {
  TimerNotifier({
    required TimerRepository timerRepository,
    required SubjectRepository subjectRepository,
    required OfflineSyncRepository offlineSync,
    required TimerSessionStore sessionStore,
    required int dayResetHour,
    required String language,
    required String timezone,
    DateTime Function()? clock,
  })  : _timerRepository = timerRepository,
        _subjectRepository = subjectRepository,
        _offlineSync = offlineSync,
        _sessionStore = sessionStore,
        _dayResetHour = dayResetHour,
        _language = language,
        _timezone = timezone,
        _now = clock ?? DateTime.now,
        super(TimerState(
          lastStudyDate: StudyDateHelper.getStudyDateString(null, dayResetHour),
        )) {
    _bootstrap();
  }

  final TimerRepository _timerRepository;
  final SubjectRepository _subjectRepository;
  final OfflineSyncRepository _offlineSync;
  final TimerSessionStore _sessionStore;
  final DateTime Function() _now;

  Timer? _ticker;
  Timer? _statsTimer;
  Timer? _dayTimer;
  int _dayResetHour;
  String _language;
  String _timezone;
  bool _syncing = false;
  bool _booted = false;

  Future<void> _bootstrap() async {
    await _loadSubjects();
    await _refreshStats();
    final snapshot = await _sessionStore.load();
    if (!mounted) return;
    if (snapshot != null) {
      _restore(snapshot);
    } else {
      _probeRemoteActivity();
    }
    _booted = true;
    _ticker ??= Timer.periodic(const Duration(seconds: 1), (_) => _tick());
    // Server truth is polled, never inferred from clock drift.
    _statsTimer ??= Timer.periodic(const Duration(seconds: 30), (_) {
      if (!state.isRunning) _refreshStats();
    });
    _dayTimer ??=
        Timer.periodic(const Duration(seconds: 20), (_) => _checkDayRollover());
  }

  // ---------------------------------------------------------------- subjects

  Future<void> _loadSubjects() async {
    try {
      final result = await _subjectRepository.fetchSubjectsData(
        language: _language,
        timezone: _timezone,
      );
      if (!mounted || result.subjects.isEmpty) return;
      final subjects = [...result.subjects]
        ..sort((a, b) => a.order.compareTo(b.order));
      final selected = _resolveSubject(state.currentSubject, subjects);
      state = state.copyWith(subjects: subjects, currentSubject: selected);
    } catch (_) {
      // Offline: keep whatever the cache returned on the previous launch.
    }
  }

  SubjectModel? _resolveSubject(
      SubjectModel? current, List<SubjectModel> subjects) {
    if (subjects.isEmpty) return null;
    if (current != null) {
      for (final s in subjects) {
        if (s.id == current.id) return s;
      }
    }
    return subjects.first;
  }

  void selectSubject(SubjectModel subject) {
    if (state.isRunning || state.isResting) return;
    state = state.copyWith(currentSubject: subject);
  }

  // ------------------------------------------------------------------ stats

  Future<void> _refreshStats() async {
    try {
      final date = StudyDateHelper.getStudyDateString(null, _dayResetHour);
      final dl = await _timerRepository.fetchDailyLogDl();
      if (!mounted) return;
      final stats = DailyStats.fromDailyLog(
        dl,
        date: date,
        knownSubjects: state.subjects,
      );
      debugPrint('[mpt] day $date total=${stats.totalMs}ms '
          'subjects=${stats.subjects.map((s) => '${s.title}:${s.ms}').join(',')}');
      state = state.copyWith(stats: stats, lastStudyDate: date);
    } catch (_) {
      // Keep the previous numbers instead of flashing zeros.
    }
  }

  Future<void> refreshStats() => _refreshStats();

  /// Detects an open session started on another device so the UI can say so.
  /// It is deliberately read-only: mirroring a remote session would make YPT
  /// count the same minutes twice once this client stops it.
  Future<void> _probeRemoteActivity() async {
    try {
      final dl = await _timerRepository.fetchDailyLogDl();
      if (!mounted || dl == null) return;
      final sessions = dl['ls'];
      if (sessions is! List || sessions.isEmpty) return;
      final last = sessions.last;
      if (last is! Map) return;
      final start = DateTime.tryParse((last['st'] ?? '').toString());
      final ms = (last['sm'] as int?) ?? 0;
      if (start == null || ms <= 0) return;
      final end = start.add(Duration(milliseconds: ms));
      final stillOpen = _now().difference(end).inSeconds.abs() < 45;
      if (stillOpen) {
        state = state.copyWith(remoteActive: true);
        _statsTimer?.cancel();
        _statsTimer =
            Timer.periodic(const Duration(seconds: 20), (_) => _refreshStats());
      }
    } catch (_) {}
  }

  void _checkDayRollover() {
    final current = StudyDateHelper.getStudyDateString(null, _dayResetHour);
    if (state.lastStudyDate.isEmpty || state.lastStudyDate == current) return;
    state = state.copyWith(
      lastStudyDate: current,
      stats: DailyStats.empty.copyWithDate(current),
    );
    _refreshStats();
    _loadSubjects();
  }

  // ---------------------------------------------------------------- session

  void _restore(TimerSessionSnapshot snapshot) {
    final now = _now();
    final subject = _findSubject(snapshot.subjectId, snapshot.subjectTitle);

    int elapsed = snapshot.accumulatedMs;
    if (!snapshot.isResting && snapshot.segmentStartAt != null) {
      final delta = now.difference(snapshot.segmentStartAt!).inMilliseconds;
      if (delta > 0) elapsed += delta;
    }
    int rest = snapshot.sessionRestMs;
    if (snapshot.isResting && snapshot.restStartAt != null) {
      final delta = now.difference(snapshot.restStartAt!).inMilliseconds;
      if (delta > 0) rest += delta;
    }

    state = state.copyWith(
      isRunning: !snapshot.isResting,
      isResting: snapshot.isResting,
      segmentStartAt: snapshot.isResting ? null : snapshot.segmentStartAt,
      clearSegmentStart: snapshot.isResting,
      accumulatedMs: snapshot.accumulatedMs,
      sessionElapsedMs: elapsed,
      sessionRestMs: rest,
      restStartAt: snapshot.isResting ? snapshot.restStartAt : null,
      clearRestStart: !snapshot.isResting,
      currentSubject: subject ?? state.currentSubject,
      segmentBaseMs: snapshot.accumulatedMs,
      lastSyncedMs: snapshot.accumulatedMs,
    );
    _accumulatedRestMs = snapshot.sessionRestMs;
    debugPrint(
        '[mpt] restored session (running=${!snapshot.isResting}, ${elapsed}ms)');
  }

  SubjectModel? _findSubject(int id, String title) {
    for (final s in state.subjects) {
      if (s.id == id) return s;
    }
    for (final s in state.subjects) {
      if (s.title == title) return s;
    }
    return null;
  }

  Future<void> _saveSnapshot() async {
    if (!mounted) return;
    final subject = state.currentSubject;
    if (subject == null || !state.hasSession) return;
    await _sessionStore.save(TimerSessionSnapshot(
      subjectId: subject.id,
      subjectTitle: subject.title,
      segmentStartAt: state.isRunning ? state.segmentStartAt : null,
      accumulatedMs: state.accumulatedMs,
      isResting: state.isResting,
      restStartAt: state.isResting ? state.restStartAt : null,
      sessionRestMs: state.sessionRestMs,
      savedAt: _now(),
    ));
  }

  void _tick() {
    if (!mounted) return;

    if (state.isResting && state.restStartAt != null) {
      final delta = _now().difference(state.restStartAt!).inMilliseconds;
      if (delta > 0) {
        state = state.copyWith(sessionRestMs: _accumulatedRestMs + delta);
      }
      return;
    }

    if (!state.isRunning || state.segmentStartAt == null) return;
    final delta = _now().difference(state.segmentStartAt!).inMilliseconds;
    if (delta < 0) return;
    state = state.copyWith(sessionElapsedMs: state.accumulatedMs + delta);
  }

  int get accumulatedRestMs => _accumulatedRestMs;
  int _accumulatedRestMs = 0;

  Future<void> start() async {
    if (state.isRunning || state.busy) return;
    final subject = state.currentSubject;
    if (subject == null) return;

    final now = _now();
    final wasResting = state.isResting;

    if (wasResting) {
      await _postRest(until: now);
    }

    state = state.copyWith(busy: true, clearNotice: true);

    String? failure;
    try {
      final result = await _timerRepository
          .startStudy(
            subjectTitle: subject.title,
            subjectId: subject.id,
            startAt: now,
          )
          .timeout(const Duration(seconds: 8),
              onTimeout: () => (ok: false, message: null));
      if (!result.ok) failure = result.message;
    } catch (e) {
      failure = e.toString();
    }
    if (!mounted) return;

    _accumulatedRestMs = 0;
    state = state.copyWith(
      busy: false,
      isRunning: true,
      isResting: false,
      segmentStartAt: now,
      restStartAt: null,
      clearRestStart: true,
      sessionRestMs: 0,
      accumulatedMs: state.sessionElapsedMs,
      segmentBaseMs: state.sessionElapsedMs,
      notice: failure == null
          ? null
          : (failure.isEmpty
              ? 'YPT did not accept the start. The timer still runs locally and will sync on stop.'
              : 'YPT: $failure'),
    );
    await _saveSnapshot();
  }

  Future<void> pause() async {
    if (!state.isRunning || state.busy) return;
    state = state.copyWith(busy: true);
    await _syncSegment();
    if (!mounted) return;

    final now = _now();
    _accumulatedRestMs = 0;
    state = state.copyWith(
      busy: false,
      isRunning: false,
      isResting: true,
      segmentStartAt: null,
      clearSegmentStart: true,
      restStartAt: now,
      sessionRestMs: 0,
      accumulatedMs: state.sessionElapsedMs,
    );
    await _saveSnapshot();
  }

  Future<void> stop() async {
    if (state.busy) return;
    state = state.copyWith(busy: true);

    if (state.isRunning) {
      await _syncSegment();
    } else if (state.isResting) {
      await _postRest(until: _now());
    }
    if (!mounted) return;

    _accumulatedRestMs = 0;
    state = state.copyWith(
      busy: false,
      isRunning: false,
      isResting: false,
      segmentStartAt: null,
      clearSegmentStart: true,
      restStartAt: null,
      clearRestStart: true,
      sessionRestMs: 0,
      sessionElapsedMs: 0,
      accumulatedMs: 0,
      lastSyncedMs: 0,
      segmentBaseMs: 0,
    );
    await _sessionStore.clear();
    await _refreshStats();
  }

  /// Pushes the segment that has not reached YPT yet. On failure the segment is
  /// queued and marked as "known" locally, so the queue is the single owner of
  /// that time and the UI never counts it twice.
  Future<void> _syncSegment() async {
    if (_syncing) return;
    final start = state.segmentStartAt;
    final elapsed = state.sessionElapsedMs;
    final segmentMs = elapsed - state.segmentBaseMs;
    final subject = state.currentSubject;
    if (start == null || subject == null || segmentMs <= 0) return;

    _syncing = true;
    final stopAt = start.add(Duration(milliseconds: segmentMs));

    try {
      final res = await _timerRepository
          .stopStudy(
            subjectTitle: subject.title,
            subjectId: subject.id,
            startAt: start,
            stopAt: stopAt,
            studyMs: segmentMs,
          )
          .timeout(const Duration(seconds: 6), onTimeout: () => null);

      if (res != null) {
        if (mounted) {
          state = state.copyWith(
            lastSyncedMs: elapsed,
            stats: _statsFromResponse(res, subject, segmentMs),
          );
        }
      } else {
        await _offlineSync.enqueueSession(
          subjectId: subject.id,
          subjectTitle: subject.title,
          startAt: start,
          stopAt: stopAt,
          studyMs: segmentMs,
        );
        if (mounted) {
          state = state.copyWith(
            lastSyncedMs: elapsed,
            notice: 'Saved offline — will sync to YPT automatically.',
          );
        }
      }
    } catch (_) {
      await _offlineSync.enqueueSession(
        subjectId: subject.id,
        subjectTitle: subject.title,
        startAt: start,
        stopAt: stopAt,
        studyMs: segmentMs,
      );
      if (mounted) {
        state = state.copyWith(
          lastSyncedMs: elapsed,
          notice: 'Saved offline — will sync to YPT automatically.',
        );
      }
    } finally {
      _syncing = false;
    }
  }

  DailyStats _statsFromResponse(
      Map<String, dynamic> res, SubjectModel subject, int elapsed) {
    final dl = res['dl'];
    if (dl is Map) {
      return DailyStats.fromDailyLog(
        Map<String, dynamic>.from(dl),
        date: state.stats.date,
        knownSubjects: state.subjects,
      );
    }
    // No totals in the response: keep local truth and bump the row.
    return state.stats.withLiveDelta(
      subjectId: subject.id,
      subjectTitle: subject.title,
      liveMs: elapsed,
    );
  }

  Future<void> _postRest({required DateTime until}) async {
    final start = state.restStartAt;
    if (start == null) return;
    final restMs = state.sessionRestMs;
    if (restMs < 5000) return;
    try {
      await _timerRepository
          .recordRest(startAt: start, stopAt: until, restMs: restMs)
          .timeout(const Duration(seconds: 4), onTimeout: () => false);
    } catch (_) {}
  }

  void applySettings(
      {required int dayResetHour,
      required String language,
      required String timezone}) {
    final resetChanged = _dayResetHour != dayResetHour;
    _dayResetHour = dayResetHour;
    if (_language != language || _timezone != timezone) {
      _language = language;
      _timezone = timezone;
      _loadSubjects();
    }
    if (resetChanged) _checkDayRollover();
    state = state.copyWith(
        lastStudyDate: StudyDateHelper.getStudyDateString(null, _dayResetHour));
  }

  void clearNotice() {
    if (state.notice != null) state = state.copyWith(clearNotice: true);
  }

  @override
  void dispose() {
    _ticker?.cancel();
    _statsTimer?.cancel();
    _dayTimer?.cancel();
    super.dispose();
  }

  String get apiTimezone => _timezone;
  String get apiLanguage => _language;
  int get dayResetHour => _dayResetHour;
  bool get isBooted => _booted;
  String get deviceModel => ApiConstants.defaultDeviceModel;
}

final timerProvider = StateNotifierProvider<TimerNotifier, TimerState>((ref) {
  final settings = ref.watch(settingsNotifierProvider);
  final notifier = TimerNotifier(
    timerRepository: ref.watch(timerRepositoryProvider),
    subjectRepository: ref.watch(subjectRepositoryProvider),
    offlineSync: ref.watch(offlineSyncRepositoryProvider),
    sessionStore: ref.watch(timerSessionStoreProvider),
    dayResetHour: settings.dayResetHour,
    language: settings.selectedLanguage,
    timezone: settings.timezone,
  );

  ref.listen<SettingsState>(settingsNotifierProvider, (prev, next) {
    if (prev?.dayResetHour == next.dayResetHour &&
        prev?.selectedLanguage == next.selectedLanguage &&
        prev?.timezone == next.timezone) {
      return;
    }
    notifier.applySettings(
      dayResetHour: next.dayResetHour,
      language: next.selectedLanguage,
      timezone: next.timezone,
    );
  });

  return notifier;
});
