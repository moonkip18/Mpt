import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Shared preferences handle, overridden in `main()` so every provider can read
/// it synchronously instead of awaiting `getInstance()` per feature.
final sharedPreferencesProvider = Provider<SharedPreferences?>((ref) => null);

class SettingsState {
  /// YPT expects a language for API payloads; the UI itself is English only.
  final String selectedLanguage;

  /// Local hour at which the study day rolls over (YPT default is 05:00).
  final int dayResetHour;

  /// IANA zone sent to YPT when a study day or session is computed.
  final String timezone;

  const SettingsState({
    this.selectedLanguage = 'en',
    this.dayResetHour = 5,
    this.timezone = 'Asia/Tehran',
  });

  SettingsState copyWith({
    String? selectedLanguage,
    int? dayResetHour,
    String? timezone,
  }) {
    return SettingsState(
      selectedLanguage: selectedLanguage ?? this.selectedLanguage,
      dayResetHour: dayResetHour ?? this.dayResetHour,
      timezone: timezone ?? this.timezone,
    );
  }
}

class SettingsNotifier extends StateNotifier<SettingsState> {
  static const _languageKey = 'mpt_language';
  static const _dayResetKey = 'mpt_day_reset_hour';
  static const _timezoneKey = 'mpt_timezone';

  final SharedPreferences? _prefs;

  SettingsNotifier(this._prefs)
      : super(SettingsState(
          selectedLanguage: _prefs?.getString(_languageKey) ?? 'en',
          dayResetHour: _prefs?.getInt(_dayResetKey) ?? 5,
          timezone: _prefs?.getString(_timezoneKey) ?? 'Asia/Tehran',
        ));

  Future<void> setLanguage(String code) async {
    if (code.isEmpty || code == state.selectedLanguage) return;
    state = state.copyWith(selectedLanguage: code);
    await _prefs?.setString(_languageKey, code);
  }

  Future<void> setDayResetHour(int hour) async {
    if (hour < 0 || hour > 23 || hour == state.dayResetHour) return;
    state = state.copyWith(dayResetHour: hour);
    await _prefs?.setInt(_dayResetKey, hour);
  }

  Future<void> setTimezone(String timezone) async {
    if (timezone.isEmpty || timezone == state.timezone) return;
    state = state.copyWith(timezone: timezone);
    await _prefs?.setString(_timezoneKey, timezone);
  }
}

final settingsNotifierProvider =
    StateNotifierProvider<SettingsNotifier, SettingsState>((ref) {
  return SettingsNotifier(ref.watch(sharedPreferencesProvider));
});
