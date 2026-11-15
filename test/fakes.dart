import 'package:mpt/data/models/subject_model.dart';
import 'package:mpt/data/repositories/subject_repository.dart';
import 'package:mpt/data/repositories/timer_repository.dart';

/// Offline stand-ins for the YPT repositories: they answer instantly and never
/// touch the network, so the timer and layout can be tested deterministically.
class FakeTimerRepository extends TimerRepository {
  FakeTimerRepository({this.startOk = true, this.stopSucceeds = true});

  bool startOk;
  bool stopSucceeds;
  int startCalls = 0;
  int stopCalls = 0;
  int restCalls = 0;
  final List<int> stopStudyMs = [];
  final List<DateTime> stopStartAts = [];

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

class FakeSubjectRepository extends SubjectRepository {
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
