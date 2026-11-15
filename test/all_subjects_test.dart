import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mpt/core/theme/mpt_theme.dart';
import 'package:mpt/data/models/subject_model.dart';
import 'package:mpt/data/repositories/offline_sync_repository.dart';
import 'package:mpt/features/settings/settings_notifier.dart';
import 'package:mpt/features/timer/home_screen.dart';
import 'package:mpt/features/timer/timer_notifier.dart';
import 'package:mpt/features/timer/timer_session_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'fakes.dart';

/// Seven subjects, while the strip only has room for about five chips.
List<SubjectModel> _manySubjects() => [
      for (var i = 1; i <= 7; i++)
        SubjectModel(
            id: i, title: 'Subject $i', colorInt: 0xFF8FA6B8 + i * 0x00010101),
    ];

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late SharedPreferences prefs;
  late TimerNotifier notifier;

  setUp(() async {
    SharedPreferences.setMockInitialValues(<String, Object>{});
    prefs = await SharedPreferences.getInstance();

    final timerRepo = FakeTimerRepository();
    notifier = TimerNotifier(
      timerRepository: timerRepo,
      subjectRepository: FakeSubjectRepository(subjects: _manySubjects()),
      offlineSync:
          OfflineSyncRepository(timerRepository: timerRepo, prefs: prefs),
      sessionStore: TimerSessionStore(prefs: prefs),
      dayResetHour: 5,
      language: 'en',
      timezone: 'Asia/Tehran',
    );
  });

  Future<void> pumpHome(WidgetTester tester) async {
    tester.view.physicalSize = const Size(1180, 760);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

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
    await tester.pump(const Duration(milliseconds: 60));
  }

  testWidgets('the chevron lists every subject, not just the visible chips',
      (tester) async {
    await pumpHome(tester);

    expect(notifier.state.subjects.length, 7);
    expect(find.byTooltip('All subjects'), findsOneWidget);

    await tester.tap(find.byTooltip('All subjects'));
    await tester.pumpAndSettle();

    expect(find.byType(PopupMenuItem<int>), findsNWidgets(7));
    for (var i = 1; i <= 7; i++) {
      expect(
        find.descendant(
          of: find.byType(PopupMenuItem<int>),
          matching: find.text('Subject $i'),
        ),
        findsOneWidget,
      );
    }
  });

  testWidgets('picking a subject from the full list selects it',
      (tester) async {
    await pumpHome(tester);

    await tester.tap(find.byTooltip('All subjects'));
    await tester.pumpAndSettle();

    await tester.tap(find.descendant(
      of: find.byType(PopupMenuItem<int>),
      matching: find.text('Subject 7'),
    ));
    await tester.pumpAndSettle();

    expect(notifier.state.currentSubject?.title, 'Subject 7');
    expect(find.byType(PopupMenuItem<int>), findsNothing);
  });
}
