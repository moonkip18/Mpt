import '../../core/api/api_client.dart';
import '../../core/api/api_exception.dart';
import '../../core/constants/api_constants.dart';
import '../../core/utils/study_date_helper.dart';
import '../../core/localization/app_translation.dart';

class TimerRepository {
  final ApiClient _apiClient;

  TimerRepository({ApiClient? apiClient})
      : _apiClient = apiClient ?? ApiClient();

  /// Returns whether YPT accepted the start plus the server message, if any.
  ///
  /// The first-generation client returned a bare bool and swallowed the reason,
  /// which made "another device is already studying" look like a network
  /// failure. The message is surfaced so the UI can say what actually happened.
  Future<({bool ok, String? message})> startStudy({
    required String subjectTitle,
    required int subjectId,
    required DateTime startAt,
  }) async {
    final response = await _apiClient.post(
      ApiConstants.studyStart,
      data: {
        'subject': subjectTitle,
        'subject_id': subjectId,
        'deviceModel': ApiConstants.defaultDeviceModel,
        'taskId': null,
      },
    );

    final data = response.data;
    if (data is Map) {
      return (
        ok: data['s'] == true,
        message: data['m']?.toString(),
      );
    }
    return (ok: false, message: null);
  }

  /// Raw `dl` block of the current study day, used for totals and for the
  /// "is the phone already studying?" probe.
  ///
  /// The payload documents `ss` both inside and outside `dl` depending on the
  /// build, so the top-level copy is folded in before returning.
  Future<Map<String, dynamic>?> fetchDailyLogDl([DateTime? date]) async {
    final log = await fetchDailyLog(date ?? DateTime.now());
    if (log == null) return null;

    final raw = log['dl'];
    final dl =
        raw is Map ? Map<String, dynamic>.from(raw) : <String, dynamic>{};
    if (dl['ss'] is! List && log['ss'] is List) dl['ss'] = log['ss'];
    if (dl['ls'] is! List && log['ls'] is List) dl['ls'] = log['ls'];
    if (dl['sm'] == null && log['sm'] != null) dl['sm'] = log['sm'];
    if (dl['rm'] == null && log['rm'] != null) dl['rm'] = log['rm'];
    return dl;
  }

  /// True when a session with this subject and start instant is already stored
  /// server-side. Used before replaying the offline queue so a stop that
  /// actually succeeded (but whose response was lost) is not recorded twice.
  Future<bool> hasRecordedSession({
    required String subjectTitle,
    required DateTime startAt,
    Duration tolerance = const Duration(minutes: 3),
  }) async {
    try {
      final dl = await fetchDailyLogDl(startAt);
      final sessions = dl?['ls'];
      if (sessions is! List) return false;
      for (final item in sessions) {
        if (item is! Map) continue;
        final name = (item['sb'] ?? '').toString();
        if (name != subjectTitle) continue;
        final raw = item['st'];
        final parsed = raw is String ? DateTime.tryParse(raw) : null;
        if (parsed == null) continue;
        if (parsed.difference(startAt).abs() <= tolerance) return true;
      }
    } catch (_) {}
    return false;
  }

  Future<Map<String, dynamic>?> stopStudy({
    required String subjectTitle,
    required int subjectId,
    required DateTime stopAt,
    required int studyMs,
    required DateTime startAt,
  }) async {
    final response = await _apiClient.post(
      ApiConstants.studyStop,
      data: {
        'subject': subjectTitle,
        'subject_id': subjectId,
        'startedAt': startAt.millisecondsSinceEpoch,
        'study_ms': studyMs,
        'deviceModel': ApiConstants.defaultDeviceModel,
      },
    );

    final data = response.data;
    if (data is Map<String, dynamic> && data['s'] == true) {
      return data;
    }
    return null;
  }

  Future<bool> recordRest({
    required DateTime startAt,
    required DateTime stopAt,
    required int restMs,
    String deviceModel = ApiConstants.defaultDeviceModel,
  }) async {
    try {
      final response = await _apiClient.post(
        ApiConstants.studyBreak,
        data: {
          'startedAt': startAt.millisecondsSinceEpoch,
          'stopAt': stopAt.millisecondsSinceEpoch,
          'rest_ms': restMs,
          'deviceModel': deviceModel,
        },
      );
      final data = response.data;
      return data is Map<String, dynamic> && data['s'] == true;
    } catch (_) {
      return false;
    }
  }

  Future<Map<String, dynamic>?> logManualStudy({
    required int subjectId,
    required String subjectTitle,
    required DateTime startAt,
    required DateTime stopAt,
    String language = ApiConstants.defaultLanguage,
    String deviceModel = ApiConstants.defaultDeviceModel,
  }) async {
    final studyMs = stopAt.difference(startAt).inMilliseconds;
    if (studyMs <= 0) {
      throw ApiException(trMsg('timer_invalid_interval',
          fallback: 'O horário de término deve ser posterior ao início'));
    }

    final response = await _apiClient.post(
      '/logs/v2/study/manual',
      data: {
        'subject_id': subjectId,
        'subject': subjectTitle,
        'startedAt': startAt.millisecondsSinceEpoch,
        'stopAt': stopAt.millisecondsSinceEpoch,
        'study_ms': studyMs,
        'deviceModel': deviceModel,
        'language': language,
      },
    );

    final data = response.data;
    if (data is Map<String, dynamic> && data['s'] == true) {
      return data;
    }
    if (data is Map<String, dynamic> && data['m'] != null) {
      throw ApiException(data['m'].toString());
    }
    throw ApiException(trMsg('timer_manual_log_failed',
        fallback: 'Falha ao registrar estudo manual'));
  }

  /// Day totals for the sidebar.
  ///
  /// YPT dates logs in the account's timezone, which may not be the machine's.
  /// Asking for a specific date can therefore return an empty day even though
  /// the phone logged hours, so an empty answer is retried without the `date`
  /// parameter and the server's own "today" is used instead.
  Future<Map<String, dynamic>?> fetchDailyLog(DateTime date) async {
    final dated = await _fetchDay(StudyDateHelper.getStudyDateString(date));
    if (_hasAnySessions(dated)) return dated;

    final undated = await _fetchDay(null);
    if (_hasAnySessions(undated)) return undated;

    return dated ?? undated;
  }

  Future<Map<String, dynamic>?> _fetchDay(String? dateStr) async {
    try {
      final path = dateStr == null ? '/logs/day' : '/logs/day?date=$dateStr';
      final response = await _apiClient.get(path);
      final data = response.data;
      if (data is Map<String, dynamic> && data['s'] == true) return data;
    } catch (_) {}
    return null;
  }

  bool _hasAnySessions(Map<String, dynamic>? log) {
    if (log == null) return false;
    final dl = log['dl'];
    final maps = <dynamic>[dl, log];
    for (final m in maps) {
      if (m is! Map) continue;
      final ls = m['ls'];
      if (ls is List && ls.isNotEmpty) return true;
      final ss = m['ss'];
      if (ss is List && ss.isNotEmpty) return true;
      final sm = m['sm'];
      if (sm is num && sm > 0) return true;
    }
    return false;
  }

  Future<DateTime> syncTime() async {
    final response = await _apiClient.get(ApiConstants.timeSync);
    final data = response.data as Map<String, dynamic>;
    if (data['timestamp'] != null) {
      return DateTime.fromMillisecondsSinceEpoch(data['timestamp'] as int);
    }
    return DateTime.now();
  }
}
