//! Port of `lib/features/settings/settings_notifier.dart`.

use crate::store::Store;

const LANGUAGE_KEY: &str = "mpt_language";
const DAY_RESET_KEY: &str = "mpt_day_reset_hour";
const TIMEZONE_KEY: &str = "mpt_timezone";

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsState {
    /// YPT expects a language for API payloads; the UI itself is English only.
    pub selected_language: String,
    /// Local hour at which the study day rolls over (YPT default is 05:00).
    pub day_reset_hour: i64,
    /// IANA zone sent to YPT when a study day or session is computed.
    pub timezone: String,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            selected_language: "en".into(),
            day_reset_hour: 5,
            timezone: "Asia/Tehran".into(),
        }
    }
}

impl SettingsState {
    pub fn from_store(store: &Store) -> Self {
        Self {
            selected_language: store
                .prefs_get_string(LANGUAGE_KEY)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "en".into()),
            day_reset_hour: store.prefs_get_int(DAY_RESET_KEY).unwrap_or(5),
            timezone: store
                .prefs_get_string(TIMEZONE_KEY)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Asia/Tehran".into()),
        }
    }

    pub fn save_language(store: &mut Store, code: &str) {
        if code.is_empty() {
            return;
        }
        store.prefs_set_string(LANGUAGE_KEY, code);
        store.flush();
    }

    pub fn save_day_reset_hour(store: &mut Store, hour: i64) {
        if !(0..=23).contains(&hour) {
            return;
        }
        store.prefs_set_int(DAY_RESET_KEY, hour);
        store.flush();
    }

    pub fn save_timezone(store: &mut Store, timezone: &str) {
        if timezone.is_empty() {
            return;
        }
        store.prefs_set_string(TIMEZONE_KEY, timezone);
        store.flush();
    }
}
