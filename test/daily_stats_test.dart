import 'package:flutter_test/flutter_test.dart';
import 'package:mpt/data/daily_stats.dart';
import 'package:mpt/data/models/subject_model.dart';
import 'package:mpt/shared/format.dart';

void main() {
  group('DailyStats', () {
    test('subject totals win over raw sessions (no double count)', () {
      final dl = <String, dynamic>{
        'sm': 7200000,
        'rm': 600,
        'ss': [
          {'id': 1, 'tt': 'Math', 'sm': 5400000},
          {'id': 2, 'tt': 'Biology', 'sm': 1800000},
        ],
        // Same day, same minutes, as raw sessions — the old parser summed both
        // and reported 3h of Math for a 1.5h day.
        'ls': [
          {'sb': 'Math', 'sm': 5400000, 'st': '2026-11-15T08:00:00+03:30'},
          {'sb': 'Biology', 'sm': 1800000, 'st': '2026-11-15T10:00:00+03:30'},
        ],
      };

      final stats = DailyStats.fromDailyLog(dl);

      expect(stats.totalMs, 7200000);
      expect(stats.subjects.length, 2);
      expect(stats.msFor(id: 1, title: 'Math'), 5400000);
      expect(stats.msFor(id: 2, title: 'Biology'), 1800000);
      expect(stats.restMs, 600000);
      expect(stats.subjects.first.title, 'Math');
    });

    test('a session logged on the phone still shows when ss only has the timer',
        () {
      final dl = <String, dynamic>{
        'sm': 7200000,
        'ss': [
          {'id': 1, 'tt': 'Math', 'sm': 3600000},
        ],
        // Manually logged from the phone: present only in the raw sessions.
        'ls': [
          {'sb': 'Math', 'sm': 3600000},
          {'sb': 'Biology', 'sm': 10800000},
        ],
      };

      final stats = DailyStats.fromDailyLog(dl);

      expect(stats.subjects.length, 2);
      expect(stats.msFor(title: 'Math'), 3600000);
      expect(stats.msFor(title: 'Biology'), 10800000);
      expect(stats.totalMs, 14400000);
    });

    test('a subject that only exists in ss still shows', () {
      final dl = <String, dynamic>{
        'ss': [
          {'id': 4, 'tt': 'Chemistry', 'sm': 900000},
        ],
        'ls': <dynamic>[],
      };

      final stats = DailyStats.fromDailyLog(dl);
      expect(stats.subjects.single.title, 'Chemistry');
      expect(stats.totalMs, 900000);
    });

    test('segments are summed inside a source, not maxed', () {
      final dl = <String, dynamic>{
        'ls': [
          {'sb': 'Math', 'sm': 600000},
          {'sb': 'Math', 'sm': 600000},
          {'sb': 'Math', 'sm': 600000},
        ],
      };

      expect(DailyStats.fromDailyLog(dl).msFor(title: 'Math'), 1800000);
    });

    test('falls back to grouping sessions when ss is absent', () {
      final dl = <String, dynamic>{
        'sm': 3600000,
        'ls': [
          {'sb': 'Math', 'sm': 1200000},
          {'sb': 'Math', 'sm': 600000},
          {'sb': 'History', 'sm': 1800000},
          {'sb': '', 'sm': 999},
        ],
      };

      final stats = DailyStats.fromDailyLog(dl);

      expect(stats.subjects.length, 2);
      expect(stats.msFor(title: 'Math'), 1800000);
      expect(stats.msFor(title: 'History'), 1800000);
    });

    test('total falls back to the sum of subject rows', () {
      final dl = <String, dynamic>{
        'ls': [
          {'sb': 'A', 'sm': 600000},
          {'sb': 'B', 'sm': 900000},
        ],
      };

      expect(DailyStats.fromDailyLog(dl).totalMs, 1500000);
    });

    test('known subjects supply colours when the server omits them', () {
      final subjects = [
        const SubjectModel(id: 7, title: 'Physics', colorInt: 0xFF00FF00),
      ];
      final dl = <String, dynamic>{
        'ss': [
          {'id': 7, 'tt': 'Physics', 'sm': 600000},
        ],
      };

      final stats = DailyStats.fromDailyLog(dl, knownSubjects: subjects);
      expect(stats.subjects.single.colorInt, 0xFF00FF00);
    });

    test('live delta lands on the running subject row and total', () {
      const base = DailyStats(
        date: '2026-11-15',
        totalMs: 3600000,
        restMs: 0,
        subjects: [
          SubjectStat(id: 1, title: 'Math', ms: 2400000),
          SubjectStat(id: 2, title: 'Biology', ms: 1200000),
        ],
      );

      final live = base.withLiveDelta(
          subjectId: 1, subjectTitle: 'Math', liveMs: 600000);

      expect(live.totalMs, 4200000);
      expect(live.msFor(id: 1, title: 'Math'), 3000000);
      expect(live.msFor(id: 2, title: 'Biology'), 1200000);
    });

    test('live delta creates a row for a subject with no logged time yet', () {
      const base = DailyStats(date: 'd', totalMs: 0, restMs: 0, subjects: []);
      final live = base.withLiveDelta(
          subjectId: 9, subjectTitle: 'Chemistry', liveMs: 65000);

      expect(live.subjects.single.title, 'Chemistry');
      expect(live.subjects.single.ms, 65000);
      expect(live.totalMs, 65000);
    });

    test('null payload never throws', () {
      expect(DailyStats.fromDailyLog(null).totalMs, 0);
    });
  });

  group('formatClock', () {
    test('drops the hour field under one hour', () {
      expect(formatClock(0), '00:00');
      expect(formatClock(65000), '01:05');
      expect(formatClock(3725000), '01:02:05');
    });

    test('formatHm and compact forms', () {
      expect(formatHm(0), '0m');
      expect(formatHm(30000), '<1m');
      expect(formatHm(3900000), '1h 5m');
      expect(formatHm(7200000), '2h');
      expect(formatHmCompact(5400000), '1.5h');
    });
  });
}
