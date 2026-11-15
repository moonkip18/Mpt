import 'dart:convert';
import 'package:shared_preferences/shared_preferences.dart';

/// Snapshot of an in-progress study session.
///
/// YPT keeps a session open on the server until an explicit stop, so losing the
/// in-memory state (crash, update, forced exit) leaves the desktop showing
/// "idle" while the phone keeps counting. Persisting the snapshot lets the app
/// rebuild the exact same session on the next launch — including the rest phase
/// and the time accumulated before a pause, which the first-generation snapshot
/// dropped.
class TimerSessionSnapshot {
  final int subjectId;
  final String subjectTitle;
  final DateTime? segmentStartAt;
  final int accumulatedMs;
  final bool isResting;
  final DateTime? restStartAt;
  final int sessionRestMs;
  final DateTime savedAt;

  const TimerSessionSnapshot({
    required this.subjectId,
    required this.subjectTitle,
    required this.segmentStartAt,
    required this.accumulatedMs,
    required this.isResting,
    required this.restStartAt,
    required this.sessionRestMs,
    required this.savedAt,
  });

  Map<String, dynamic> toJson() => {
        'subject_id': subjectId,
        'subject_title': subjectTitle,
        'segment_start_at': segmentStartAt?.toUtc().millisecondsSinceEpoch,
        'accumulated_ms': accumulatedMs,
        'is_resting': isResting,
        'rest_start_at': restStartAt?.toUtc().millisecondsSinceEpoch,
        'session_rest_ms': sessionRestMs,
        'saved_at': savedAt.toUtc().millisecondsSinceEpoch,
      };

  factory TimerSessionSnapshot.fromJson(Map<String, dynamic> json) {
    DateTime? readDate(dynamic raw) {
      if (raw is int) {
        return DateTime.fromMillisecondsSinceEpoch(raw, isUtc: true).toLocal();
      }
      return null;
    }

    return TimerSessionSnapshot(
      subjectId: json['subject_id'] as int? ?? -1,
      subjectTitle: json['subject_title'] as String? ?? '',
      segmentStartAt: readDate(json['segment_start_at']),
      accumulatedMs: json['accumulated_ms'] as int? ?? 0,
      isResting: json['is_resting'] as bool? ?? false,
      restStartAt: readDate(json['rest_start_at']),
      sessionRestMs: json['session_rest_ms'] as int? ?? 0,
      savedAt: readDate(json['saved_at']) ?? DateTime.now(),
    );
  }
}

class TimerSessionStore {
  static const String storageKey = 'mpt_active_timer_session';

  final SharedPreferences? _prefs;

  TimerSessionStore({SharedPreferences? prefs}) : _prefs = prefs;

  Future<SharedPreferences> _getPrefs() async =>
      _prefs ?? await SharedPreferences.getInstance();

  Future<void> save(TimerSessionSnapshot snapshot) async {
    try {
      final prefs = await _getPrefs();
      await prefs.setString(storageKey, jsonEncode(snapshot.toJson()));
    } catch (_) {}
  }

  Future<TimerSessionSnapshot?> load() async {
    try {
      final prefs = await _getPrefs();
      final raw = prefs.getString(storageKey);
      if (raw == null || raw.isEmpty) return null;
      final decoded = jsonDecode(raw);
      if (decoded is! Map<String, dynamic>) return null;
      final snapshot = TimerSessionSnapshot.fromJson(decoded);

      final age = DateTime.now().difference(snapshot.savedAt);
      // A snapshot from days ago must not resurrect a session and inflate the
      // reported study time.
      if (age.isNegative || age > const Duration(hours: 24)) {
        await clear();
        return null;
      }
      return snapshot;
    } catch (_) {
      return null;
    }
  }

  Future<void> clear() async {
    try {
      final prefs = await _getPrefs();
      await prefs.remove(storageKey);
    } catch (_) {}
  }
}
