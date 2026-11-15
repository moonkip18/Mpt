import 'package:flutter_test/flutter_test.dart';
import 'package:mpt/data/models/subject_model.dart';
import 'package:mpt/data/repositories/offline_sync_repository.dart';
import 'package:mpt/data/repositories/subject_repository.dart';
import 'package:mpt/data/repositories/timer_repository.dart';
import 'package:mpt/features/timer/timer_notifier.dart';
import 'package:mpt/features/timer/timer_session_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

class _Clock {
  _Clock(this.value);
  DateTime value;
  DateTime now() => value;
  void advance(Duration d) => value = value.add(d);
}

class _FakeTimerRepository extends TimerRepository {
  bool startOk = true;
  bool stopSucceeds = true;
  int startCalls = 0;
  int stopCalls = 0;
  int restCalls = 0;
  final List<int> stopStudyMs = [];
  final List<DateTime> stopStartAts = [];
  Map<String, dynamic> dailyLog = <String, dynamic>{
    'sm': 0,
    'rm': 0,
    'ls': <dynamic>[]
  };

  @override
  Future<({bool ok, String? message})> startStudy({
    required String subjectTitle,
    required int subjectId,
    required DateTime startAt,
  }) async {
    startCalls++;
    return (
      ok: startOk,
      message: startOk ? null : 'already studying on another device'
    );
  }

  @override
  Future<Map<String, dynamic>?> stopStudy({
    required String subjectTitle,
    required int subjectId,
    required DateTime stopAt,
    required int studyMs,
    required DateTime startAt,
  }) async {
    stopCalls++;
    stopStudyMs.add(studyMs);
    stopStartAts.add(startAt);
    if (!stopSucceeds) return null;
    return <String, dynamic>{'s': true, 'dl': dailyLog};
  }

  @override
  Future<bool> recordRest({
    required DateTime startAt,
    required DateTime stopAt,
    required int restMs,
    String deviceModel = 'Desktop',
  }) async {
    restCalls++;
    return true;
  }

  @override
  Future<Map<String, dynamic>?> fetchDailyLog(DateTime date) async =>
      <String, dynamic>{'s': true, 'dl': dailyLog};

  @override
  Future<Map<String, dynamic>?> fetchDailyLogDl([DateTime? date]) async =>
      dailyLog;

  @override
  Future<bool> hasRecordedSession({
    required String subjectTitle,
    required DateTime startAt,
    Duration tolerance = const Duration(minutes: 3),
  }) async =>
      false;
}

class _FakeSubjectRepository extends SubjectRepository {
  @override
  Future<SubjectFetchResult> fetchSubjectsData({
    String language = 'en',
    String timezone = 'Asia/Tehran',
    int version = 810041,
  }) async {
    return const SubjectFetchResult(
      subjects: [
        SubjectModel(id: 1, title: 'Math', colorInt: 0xFF8FA6B8),
        SubjectModel(id: 2, title: 'Biology', colorInt: 0xFF8FB89B),
      ],
      todayTotalMs: 0,
      todayRestMs: 0,
    );
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late SharedPreferences prefs;
  late _FakeTimerRepository timer;
  late _FakeSubjectRepository subjects;
  late OfflineSyncRepository offline;
  late _Clock clock;

  setUp(() async {
    SharedPreferences.setMockInitialValues(<String, Object>{});
    prefs = await SharedPreferences.getInstance();
    timer = _FakeTimerRepository();
    subjects = _FakeSubjectRepository();
    offline = OfflineSyncRepository(timerRepository: timer, prefs: prefs);
    clock = _Clock(DateTime(2026, 11, 15, 9));
  });

  TimerNotifier build() => TimerNotifier(
        timerRepository: timer,
        subjectRepository: subjects,
        offlineSync: offline,
        sessionStore: TimerSessionStore(prefs: prefs),
        dayResetHour: 5,
        language: 'en',
        timezone: 'Asia/Tehran',
        clock: clock.now,
      );

  testWidgets('pause and resume record each segment once', (tester) async {
    final notifier = build();
    await tester.pump();

    expect(notifier.state.subjects.length, 2);
    expect(notifier.state.currentSubject?.title, 'Math');

    await notifier.start();
    expect(notifier.state.isRunning, isTrue);
    expect(timer.startCalls, 1);

    clock.advance(const Duration(seconds: 90));
    await tester.pump(const Duration(seconds: 1));
    expect(notifier.state.sessionElapsedMs, 90000);

    await notifier.pause();
    expect(notifier.state.isResting, isTrue);
    expect(notifier.state.isRunning, isFalse);
    expect(timer.stopCalls, 1);
    expect(timer.stopStudyMs.single, 90000);
    final elapsedAtPause = notifier.state.sessionElapsedMs;

    clock.advance(const Duration(seconds: 30));
    await tester.pump(const Duration(seconds: 1));
    expect(notifier.state.sessionRestMs, greaterThanOrEqualTo(30000));

    await notifier.start();
    expect(notifier.state.isRunning, isTrue);
    expect(timer.startCalls, 2);
    expect(timer.restCalls, 1, reason: 'rest is reported when study resumes');

    clock.advance(const Duration(seconds: 60));
    await tester.pump(const Duration(seconds: 1));
    expect(notifier.state.sessionElapsedMs, elapsedAtPause + 60000);

    await notifier.stop();
    expect(timer.stopCalls, 2);
    expect(
      timer.stopStudyMs.last,
      60000,
      reason: 'only the resumed segment is sent, not the whole session again',
    );
    expect(notifier.state.sessionElapsedMs, 0);
    expect(notifier.state.isRunning, isFalse);
    expect(notifier.state.displayTotalMs, notifier.state.stats.totalMs);

    notifier.dispose();
  });

  testWidgets('start survives a rejected server call and keeps the timer local',
      (tester) async {
    timer.startOk = false;
    final notifier = build();
    await tester.pump();

    await notifier.start();

    expect(notifier.state.isRunning, isTrue);
    expect(notifier.state.notice, contains('already studying'));
    notifier.dispose();
  });

  testWidgets('failed stop is queued and never counted twice locally',
      (tester) async {
    timer.stopSucceeds = false;
    final notifier = build();
    await tester.pump();

    await notifier.start();
    clock.advance(const Duration(minutes: 5));
    await tester.pump(const Duration(seconds: 1));
    expect(notifier.state.sessionElapsedMs, 300000);

    await notifier.stop();

    final pending = await offline.getPendingSessions();
    expect(pending.length, 1);
    expect(pending.single.studyMs, 300000);
    expect(pending.single.subjectTitle, 'Math');
    expect(notifier.state.notice, contains('offline'));
    expect(notifier.state.displayTotalMs, notifier.state.stats.totalMs);
    notifier.dispose();
  });

  testWidgets('session survives a restart without losing paused time',
      (tester) async {
    final notifier = build();
    await tester.pump();
    await notifier.start();
    clock.advance(const Duration(minutes: 10));
    await tester.pump(const Duration(seconds: 1));
    await notifier.pause();
    final pausedElapsed = notifier.state.sessionElapsedMs;
    notifier.dispose();

    final restored = build();
    await tester.pump();

    expect(restored.state.isResting, isTrue);
    expect(restored.state.sessionElapsedMs, pausedElapsed);
    restored.dispose();
  });
}
