import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mpt/core/theme/mpt_theme.dart';
import 'package:mpt/data/models/subject_model.dart';
import 'package:mpt/data/repositories/offline_sync_repository.dart';
import 'package:mpt/data/repositories/subject_repository.dart';
import 'package:mpt/data/repositories/timer_repository.dart';
import 'package:mpt/features/settings/settings_notifier.dart';
import 'package:mpt/features/auth/login_screen.dart';
import 'package:mpt/features/timer/home_screen.dart';
import 'package:mpt/features/timer/mini_player_notifier.dart';
import 'package:mpt/features/timer/timer_notifier.dart';
import 'package:mpt/features/timer/timer_session_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

class _FakeTimerRepository extends TimerRepository {
  Map<String, dynamic> dailyLog = <String, dynamic>{
    'sm': 5400000,
    'rm': 0,
    'ss': [
      {'id': 1, 'tt': 'Mathematics', 'sm': 3600000, 'co': 0xFF8FA6B8},
      {'id': 2, 'tt': 'Biology', 'sm': 1800000, 'co': 0xFF8FB89B},
    ],
    'ls': <dynamic>[],
  };

  @override
  Future<({bool ok, String? message})> startStudy({
    required String subjectTitle,
    required int subjectId,
    required DateTime startAt,
  }) async =>
      (ok: true, message: null);

  @override
  Future<Map<String, dynamic>?> stopStudy({
    required String subjectTitle,
    required int subjectId,
    required DateTime stopAt,
    required int studyMs,
    required DateTime startAt,
  }) async =>
      <String, dynamic>{'s': true, 'dl': dailyLog};

  @override
  Future<bool> recordRest({
    required DateTime startAt,
    required DateTime stopAt,
    required int restMs,
    String deviceModel = 'Desktop',
  }) async =>
      true;

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
        SubjectModel(id: 1, title: 'Mathematics', colorInt: 0xFF8FA6B8),
        SubjectModel(id: 2, title: 'Biology', colorInt: 0xFF8FB89B),
        SubjectModel(
            id: 3, title: 'History of the Ancient World', colorInt: 0xFFB89B8F),
      ],
      todayTotalMs: 5400000,
      todayRestMs: 0,
    );
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late SharedPreferences prefs;

  setUp(() async {
    SharedPreferences.setMockInitialValues(<String, Object>{});
    prefs = await SharedPreferences.getInstance();
  });

  Future<void> setSize(WidgetTester tester, Size size) async {
    tester.view.physicalSize = size;
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
  }

  testWidgets('login screen lays out at the minimum window size',
      (tester) async {
    await setSize(tester, const Size(900, 560));

    await tester.pumpWidget(
      ProviderScope(
        overrides: [sharedPreferencesProvider.overrideWithValue(prefs)],
        child: MaterialApp(
          theme: MptTheme.build(),
          home: const LoginScreen(),
        ),
      ),
    );
    await tester.pump();

    expect(find.text('Sign in to YPT'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('full study screen lays out at the minimum window size',
      (tester) async {
    await setSize(tester, const Size(900, 560));

    final timerRepo = _FakeTimerRepository();
    final notifier = TimerNotifier(
      timerRepository: timerRepo,
      subjectRepository: _FakeSubjectRepository(),
      offlineSync:
          OfflineSyncRepository(timerRepository: timerRepo, prefs: prefs),
      sessionStore: TimerSessionStore(prefs: prefs),
      dayResetHour: 5,
      language: 'en',
      timezone: 'Asia/Tehran',
    );

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          sharedPreferencesProvider.overrideWithValue(prefs),
          timerProvider.overrideWith((ref) => notifier),
        ],
        child: MaterialApp(theme: MptTheme.build(), home: const HomeScreen()),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));

    expect(find.text('Mathematics'), findsWidgets);
    expect(find.text('Start'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('mini player fits a 360x148 window', (tester) async {
    await setSize(tester, const Size(360, 148));

    final timerRepo = _FakeTimerRepository();
    final notifier = TimerNotifier(
      timerRepository: timerRepo,
      subjectRepository: _FakeSubjectRepository(),
      offlineSync:
          OfflineSyncRepository(timerRepository: timerRepo, prefs: prefs),
      sessionStore: TimerSessionStore(prefs: prefs),
      dayResetHour: 5,
      language: 'en',
      timezone: 'Asia/Tehran',
    );

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          sharedPreferencesProvider.overrideWithValue(prefs),
          timerProvider.overrideWith((ref) => notifier),
          miniPlayerProvider
              .overrideWith((ref) => MiniPlayerNotifier(initial: true)),
        ],
        child: MaterialApp(theme: MptTheme.build(), home: const HomeScreen()),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));

    expect(tester.takeException(), isNull);
  });
}
