//! Port of `lib/data/repositories/timer_repository.dart`.
//!
//! The API surface is a trait so tests can substitute fakes, mirroring the
//! Dart project's `FakeTimerRepository`.

use chrono::{DateTime, Duration, Local};
use serde_json::Value;

use crate::api::{ApiClient, ApiError};
use crate::constants::{STUDY_BREAK, STUDY_START, STUDY_STOP, TIME_SYNC};
use crate::localization::tr_msg;
use crate::store::Store;
use crate::utils::study_date::{get_study_date_string, DEFAULT_RESET_HOUR};

use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct TimerStartResult {
    pub ok: bool,
    pub message: Option<String>,
}

/// Everything the timer feature needs from the YPT study endpoints.
pub trait TimerApi: Send + Sync {
    /// POST /study/start — returns whether YPT accepted it plus the message.
    fn start_study(&self, subject_title: &str, subject_id: i64, start_at: DateTime<Local>)
        -> TimerStartResult;

    /// POST /study/stop — Ok(Some(data)) only when the server reports success.
    fn stop_study(
        &self,
        subject_title: &str,
        subject_id: i64,
        stop_at: DateTime<Local>,
        study_ms: i64,
        start_at: DateTime<Local>,
    ) -> Result<Option<Value>, String>;

    /// POST /study/study-plan/rest — best effort; failure is not surfaced.
    fn record_rest(
        &self,
        start_at: DateTime<Local>,
        stop_at: DateTime<Local>,
        rest_ms: i64,
    ) -> bool;

    /// Raw `dl` block of the current study day, with top-level `ss`/`ls`/`sm`/`rm`
    /// folded in when the build only reports them outside `dl`.
    fn fetch_daily_log_dl(&self, date: Option<DateTime<Local>>)
        -> Result<Option<Value>, String>;

    /// True when a session with this subject and start instant is already stored
    /// server-side (offline-queue duplicate guard; 3-minute tolerance).
    fn has_recorded_session(
        &self,
        subject_title: &str,
        start_at: DateTime<Local>,
        tolerance: Duration,
    ) -> bool;
}

pub struct TimerRepository {
    api: Arc<Mutex<ApiClient>>,
}

impl TimerRepository {
    pub fn new(store: &Arc<Mutex<Store>>) -> Self {
        Self {
            api: Arc::new(Mutex::new(ApiClient::new(store.clone()))),
        }
    }

    fn with_client<F, T>(&self, f: F) -> Result<T, ApiError>
    where
        F: FnOnce(&ApiClient) -> Result<T, ApiError>,
    {
        let client = self.api.lock().unwrap();
        f(&client)
    }

    /// GET /logs/manual-study — manual entry (feature parity with Desky).
    pub fn log_manual_study(
        &self,
        subject_id: i64,
        subject_title: &str,
        start_at: DateTime<Local>,
        stop_at: DateTime<Local>,
        language: &str,
    ) -> Result<Option<Value>, ApiError> {
        let study_ms = (stop_at - start_at).num_milliseconds();
        if study_ms <= 0 {
            return Err(ApiError::Decode(tr_msg(
                "timer_invalid_interval",
                None,
                Some("O horário de término deve ser posterior ao início"),
            )));
        }
        let body = serde_json::json!({
            "subject_id": subject_id,
            "subject": subject_title,
            "startedAt": start_at.timestamp_millis(),
            "stopAt": stop_at.timestamp_millis(),
            "study_ms": study_ms,
            "deviceModel": "Desktop",
            "language": language,
        });
        let resp = self.with_client(|c| c.post("/logs/v2/study/manual", &body, None, false))?;
        if resp.data.get("s").and_then(Value::as_bool).unwrap_or(false) {
            return Ok(Some(resp.data));
        }
        if let Some(m) = resp.data.get("m") {
            return Err(ApiError::Decode(m.to_string()));
        }
        Err(ApiError::Decode(tr_msg(
            "timer_manual_log_failed",
            None,
            Some("Falha ao registrar estudo manual"),
        )))
    }

    /// GET /time/ — time sync against the server clock.
    pub fn sync_time(&self) -> Result<DateTime<Local>, ApiError> {
        let resp = self.with_client(|c| c.get(TIME_SYNC, None, None))?;
        if let Some(ts) = resp.data.get("timestamp").and_then(Value::as_i64) {
            Ok(DateTime::from_timestamp_millis(ts)
                .map(|d| d.with_timezone(&Local))
                .unwrap_or_else(Local::now))
        } else {
            Ok(Local::now())
        }
    }

    /// `/logs/day` for the study day of `date`.
    ///
    /// YPT dates logs in the account's timezone, which may not be the machine's,
    /// so an empty dated answer is retried without `date` and the server's own
    /// "today" wins if it has sessions.
    pub fn fetch_daily_log(&self, date: DateTime<Local>) -> Option<Value> {
        let date_str = get_study_date_string(date, DEFAULT_RESET_HOUR);
        let dated = self.fetch_day(Some(&date_str));
        if Self::has_any_sessions(dated.as_ref()) {
            return dated;
        }
        let undated = self.fetch_day(None);
        if Self::has_any_sessions(undated.as_ref()) {
            return undated;
        }
        dated.or(undated)
    }

    fn fetch_day(&self, date_str: Option<&str>) -> Option<Value> {
        let path = match date_str {
            None => "/logs/day".to_string(),
            Some(d) => format!("/logs/day?date={d}"),
        };
        let resp = self.with_client(|c| c.get(&path, None, None)).ok()?;
        if resp.data.get("s").and_then(Value::as_bool).unwrap_or(false) {
            Some(resp.data)
        } else {
            None
        }
    }

    fn has_any_sessions(log: Option<&Value>) -> bool {
        let Some(log) = log else { return false; };
        for m in [log.get("dl"), Some(log)] {
            let Some(m) = m else { continue; };
            if let Some(ls) = m.get("ls").and_then(Value::as_array) {
                if !ls.is_empty() {
                    return true;
                }
            }
            if let Some(ss) = m.get("ss").and_then(Value::as_array) {
                if !ss.is_empty() {
                    return true;
                }
            }
            if let Some(sm) = m.get("sm").and_then(Value::as_f64) {
                if sm > 0.0 {
                    return true;
                }
            }
        }
        false
    }
}

impl TimerApi for TimerRepository {
    fn start_study(
        &self,
        subject_title: &str,
        subject_id: i64,
        _start_at: DateTime<Local>,
    ) -> TimerStartResult {
        let body = serde_json::json!({
            "subject": subject_title,
            "subject_id": subject_id,
            "deviceModel": "Desktop",
            "taskId": null,
        });
        match self.with_client(|c| c.post(STUDY_START, &body, None, false)) {
            Ok(resp) => match &resp.data {
                Value::Object(_) => TimerStartResult {
                    ok: resp.data.get("s").and_then(Value::as_bool).unwrap_or(false),
                    message: resp.data.get("m").and_then(Value::as_str).map(String::from),
                },
                _ => TimerStartResult { ok: false, message: None },
            },
            // Network failure: local timer still runs; caller surfaces the reason.
            Err(e) => TimerStartResult { ok: false, message: Some(e.to_string()) },
        }
    }

    fn stop_study(
        &self,
        subject_title: &str,
        subject_id: i64,
        stop_at: DateTime<Local>,
        study_ms: i64,
        start_at: DateTime<Local>,
    ) -> Result<Option<Value>, String> {
        let body = serde_json::json!({
            "subject": subject_title,
            "subject_id": subject_id,
            "startedAt": start_at.timestamp_millis(),
            "stopAt": stop_at.timestamp_millis(),
            "study_ms": study_ms,
            "deviceModel": "Desktop",
        });
        let resp = self
            .with_client(|c| c.post(STUDY_STOP, &body, None, false))
            .map_err(|e| e.to_string())?;
        if resp.data.get("s").and_then(Value::as_bool).unwrap_or(false) {
            Ok(Some(resp.data))
        } else {
            Ok(None)
        }
    }

    fn record_rest(
        &self,
        start_at: DateTime<Local>,
        stop_at: DateTime<Local>,
        rest_ms: i64,
    ) -> bool {
        let body = serde_json::json!({
            "startedAt": start_at.timestamp_millis(),
            "stopAt": stop_at.timestamp_millis(),
            "rest_ms": rest_ms,
            "deviceModel": "Desktop",
        });
        self.with_client(|c| c.post(STUDY_BREAK, &body, None, false))
            .map(|resp| resp.data.get("s").and_then(Value::as_bool).unwrap_or(false))
            .unwrap_or(false)
    }

    fn fetch_daily_log_dl(
        &self,
        date: Option<DateTime<Local>>,
    ) -> Result<Option<Value>, String> {
        let now = date.unwrap_or_else(Local::now);
        let log = self.fetch_daily_log(now);
        let Some(log) = log else { return Ok(None); };
        let mut dl = match log.get("dl") {
            Some(Value::Object(map)) => Value::Object(map.clone()),
            _ => Value::Object(Default::default()),
        };
        if !dl.get("ss").is_some_and(Value::is_array) {
            if let Some(ss) = log.get("ss") {
                dl["ss"] = ss.clone();
            }
        }
        if !dl.get("ls").is_some_and(Value::is_array) {
            if let Some(ls) = log.get("ls") {
                dl["ls"] = ls.clone();
            }
        }
        if dl.get("sm").is_none() {
            if let Some(sm) = log.get("sm") {
                dl["sm"] = sm.clone();
            }
        }
        if dl.get("rm").is_none() {
            if let Some(rm) = log.get("rm") {
                dl["rm"] = rm.clone();
            }
        }
        Ok(Some(dl))
    }

    fn has_recorded_session(
        &self,
        subject_title: &str,
        start_at: DateTime<Local>,
        tolerance: Duration,
    ) -> bool {
        let Ok(Some(dl)) = self.fetch_daily_log_dl(Some(start_at)) else {
            return false;
        };
        let Some(ls) = dl.get("ls").and_then(Value::as_array) else {
            return false;
        };
        for item in ls {
            let name = item.get("sb").and_then(Value::as_str).unwrap_or("");
            if name != subject_title {
                continue;
            }
            let Some(raw) = item.get("st").and_then(Value::as_str) else { continue; };
            let Ok(parsed) = DateTime::parse_from_rfc3339(raw) else { continue; };
            let parsed = parsed.with_timezone(&Local);
            if (parsed - start_at).num_milliseconds().abs() <= tolerance.num_milliseconds() {
                return true;
            }
        }
        false
    }
}
