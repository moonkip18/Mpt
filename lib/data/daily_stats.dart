import '../../data/models/subject_model.dart';

/// One subject's total for the current study day.
class SubjectStat {
  final int? id;
  final String title;
  final int ms;
  final int colorInt;

  const SubjectStat({
    this.id,
    required this.title,
    required this.ms,
    this.colorInt = 0xFFDFE3E8,
  });

  SubjectStat copyWith({int? ms}) => SubjectStat(
        id: id,
        title: title,
        ms: ms ?? this.ms,
        colorInt: colorInt,
      );
}

/// YPT's answer for "what did today look like", parsed once so the sidebar and
/// the header never disagree.
///
/// The daily payload carries two sources of truth:
///   * `dl.ss` — per-subject totals computed by the server;
///   * `dl.ls` — the raw session list (each with `sb` and `sm`).
/// Reading both and adding them up double counts every subject, so `ss` wins
/// whenever it is present and `ls` is only used as a fallback.
class DailyStats {
  final String date;
  final int totalMs;
  final int restMs;
  final List<SubjectStat> subjects;

  const DailyStats({
    required this.date,
    required this.totalMs,
    required this.restMs,
    required this.subjects,
  });

  static const empty = DailyStats(
    date: '',
    totalMs: 0,
    restMs: 0,
    subjects: [],
  );

  bool get isEmpty => totalMs == 0 && subjects.isEmpty;

  int msFor({int? id, required String title}) {
    for (final s in subjects) {
      if (id != null && s.id != null && s.id == id) return s.ms;
      if (s.title == title) return s.ms;
    }
    return 0;
  }

  factory DailyStats.fromDailyLog(
    Map<String, dynamic>? dl, {
    String date = '',
    List<SubjectModel> knownSubjects = const [],
  }) {
    if (dl == null) return empty.copyWithDate(date);

    final totalMs = _asInt(dl['sm'] ?? dl['tp']);
    final restRaw = _asInt(dl['rm'] ?? dl['rt']);
    final restMs = restRaw > 0 && restRaw < 500000 ? restRaw * 1000 : restRaw;

    final colourByName = <String, int>{
      for (final s in knownSubjects) s.title: s.colorInt,
    };
    final colourById = <int, int>{
      for (final s in knownSubjects) s.id: s.colorInt,
    };

    // `ss` carries server totals per subject, `ls` the raw sessions. Neither is
    // guaranteed to hold everything — a session logged on the phone can appear
    // in only one of them — so each list is summed on its own and then the
    // larger total wins per subject. Summing inside a source keeps split
    // segments, and taking the max across sources still counts a segment that
    // both lists happen to contain exactly once.
    final ssTotals = <String, SubjectStat>{};
    final lsTotals = <String, SubjectStat>{};

    void addTo(Map<String, SubjectStat> into, String rawTitle, int? id, int ms,
        int? colour) {
      final title = rawTitle.trim();
      if (title.isEmpty || ms <= 0) return;
      final existing = into[title];
      final resolved = colour ??
          (id != null ? colourById[id] : null) ??
          colourByName[title] ??
          _fallbackColour(title);
      into[title] = existing == null
          ? SubjectStat(id: id, title: title, ms: ms, colorInt: resolved)
          : SubjectStat(
              id: existing.id ?? id,
              title: title,
              ms: existing.ms + ms,
              colorInt: existing.colorInt,
            );
    }

    final rawSubjects = dl['ss'];
    if (rawSubjects is List) {
      for (final item in rawSubjects) {
        if (item is! Map) continue;
        addTo(
          ssTotals,
          (item['tt'] ?? item['sb'] ?? '').toString(),
          _asNullableInt(item['id']),
          _asInt(item['sm']),
          _asNullableInt(item['co']),
        );
      }
    }

    final rawSessions = dl['ls'];
    if (rawSessions is List) {
      for (final item in rawSessions) {
        if (item is! Map) continue;
        addTo(
          lsTotals,
          (item['sb'] ?? item['tt'] ?? '').toString(),
          _asNullableInt(item['id'] ?? item['subjectId']),
          _asInt(item['sm']),
          _asNullableInt(item['co']),
        );
      }
    }

    final merged = <String, SubjectStat>{};
    for (final entry in [...ssTotals.values, ...lsTotals.values]) {
      final existing = merged[entry.title];
      merged[entry.title] = existing == null
          ? entry
          : SubjectStat(
              id: existing.id ?? entry.id,
              title: entry.title,
              ms: existing.ms > entry.ms ? existing.ms : entry.ms,
              colorInt: existing.colorInt,
            );
    }

    final stats = merged.values.toList()..sort((a, b) => b.ms.compareTo(a.ms));

    final sumSubjects = stats.fold<int>(0, (sum, s) => sum + s.ms);
    return DailyStats(
      date: date,
      totalMs: totalMs > sumSubjects ? totalMs : sumSubjects,
      restMs: restMs,
      subjects: stats,
    );
  }

  DailyStats copyWithDate(String newDate) => DailyStats(
        date: newDate,
        totalMs: totalMs,
        restMs: restMs,
        subjects: subjects,
      );

  /// Adds an unsynced remainder to the subject's row without touching the
  /// server numbers, so the sidebar stays honest while a session runs.
  DailyStats withLiveDelta(
      {int? subjectId, String? subjectTitle, int liveMs = 0}) {
    if (liveMs <= 0 || (subjectId == null && subjectTitle == null)) return this;

    var matched = false;
    final updated = <SubjectStat>[];
    for (final s in subjects) {
      final isSame = (subjectId != null && s.id == subjectId) ||
          (subjectTitle != null && s.title == subjectTitle);
      if (isSame && !matched) {
        matched = true;
        updated.add(s.copyWith(ms: s.ms + liveMs));
      } else {
        updated.add(s);
      }
    }
    if (!matched) {
      updated.add(SubjectStat(
        id: subjectId,
        title: subjectTitle ?? 'Study',
        ms: liveMs,
      ));
    }
    updated.sort((a, b) => b.ms.compareTo(a.ms));
    return DailyStats(
      date: date,
      totalMs: totalMs + liveMs,
      restMs: restMs,
      subjects: updated,
    );
  }

  static int _asInt(dynamic v) {
    if (v is int) return v;
    if (v is double) return v.round();
    if (v is String) return int.tryParse(v) ?? 0;
    return 0;
  }

  static int? _asNullableInt(dynamic v) {
    if (v is int) return v;
    if (v is double) return v.round();
    if (v is String) return int.tryParse(v);
    return null;
  }

  static int _fallbackColour(String title) {
    var hash = 0;
    for (final unit in title.codeUnits) {
      hash = (hash * 31 + unit) & 0x7fffffff;
    }
    const palette = [
      0xFF8FA6B8,
      0xFFB89B8F,
      0xFF8FB89B,
      0xFFB8B08F,
      0xFFA68FB8,
      0xFF8FA6B8,
      0xFFB88FA6,
    ];
    return palette[hash % palette.length];
  }
}
