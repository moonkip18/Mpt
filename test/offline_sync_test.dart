import 'package:flutter_test/flutter_test.dart';
import 'package:mpt/data/repositories/offline_sync_repository.dart';
import 'package:mpt/data/repositories/timer_repository.dart';
import 'package:shared_preferences/shared_preferences.dart';

class _FakeTimerRepository extends TimerRepository {
  bool recorded = false;
  bool stopSucceeds = true;
  int stopCalls = 0;

  @override
  Future<bool> hasRecordedSession({
    required String subjectTitle,
    required DateTime startAt,
    Duration tolerance = const Duration(minutes: 3),
  }) async {
    return recorded;
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
    return stopSucceeds ? <String, dynamic>{'s': true} : null;
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late SharedPreferences prefs;
  late _FakeTimerRepository timer;

  setUp(() async {
    SharedPreferences.setMockInitialValues(<String, Object>{});
    prefs = await SharedPreferences.getInstance();
    timer = _FakeTimerRepository();
  });

  OfflineSyncRepository repo() =>
      OfflineSyncRepository(timerRepository: timer, prefs: prefs);

  Future<void> enqueueOne(OfflineSyncRepository queue) => queue.enqueueSession(
        subjectId: 3,
        subjectTitle: 'Math',
        startAt: DateTime(2026, 11, 15, 9),
        stopAt: DateTime(2026, 11, 15, 9, 30),
        studyMs: 1800000,
      );

  test('queued session is replayed and removed on success', () async {
    final queue = repo();
    await enqueueOne(queue);
    expect((await queue.getPendingSessions()).length, 1);

    final result = await queue.syncAllPending();

    expect(result.totalSynced, 1);
    expect(result.remaining, 0);
    expect(timer.stopCalls, 1);
    expect(await queue.getPendingSessions(), isEmpty);
  });

  test('session already on the server is dropped without a second POST',
      () async {
    timer.recorded = true;
    final queue = repo();
    await enqueueOne(queue);

    final result = await queue.syncAllPending();

    expect(result.totalSynced, 1);
    expect(result.remaining, 0);
    expect(timer.stopCalls, 0,
        reason: 'replaying would double count the session');
  });

  test('rejected session stays queued with a retry count', () async {
    timer.stopSucceeds = false;
    final queue = repo();
    await enqueueOne(queue);

    final result = await queue.syncAllPending();

    expect(result.totalFailed, 1);
    expect(result.remaining, 1);
    final pending = await queue.getPendingSessions();
    expect(pending.single.retryCount, 1);
  });

  test('a failed retry keeps the original start instant', () async {
    timer.stopSucceeds = false;
    final queue = repo();
    await enqueueOne(queue);
    await queue.syncAllPending();
    await queue.syncAllPending();

    final pending = await queue.getPendingSessions();
    expect(pending.single.retryCount, 2);
    expect(pending.single.startAt, DateTime(2026, 11, 15, 9));
  });

  test('empty queue is a no-op', () async {
    final result = await repo().syncAllPending();
    expect(result.totalSynced, 0);
    expect(timer.stopCalls, 0);
  });
}
