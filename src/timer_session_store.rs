//! Port of `lib/features/timer/timer_session_store.dart`.
//!
//! YPT keeps a session open until an explicit stop, so the in-progress session
//! is snapshotted to disk and rebuilt after a crash or restart — including the
//! rest phase and time accumulated before a pause.

use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::store::Store;

const STORAGE_KEY: &str = "mpt_active_timer_session";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimerSessionSnapshot {
    pub subject_id: i64,
    pub subject_title: String,
    /// UTC epoch millis; absent while resting.
    pub segment_start_at: Option<i64>,
    pub accumulated_ms: i64,
    pub is_resting: bool,
    /// UTC epoch millis; absent while studying.
    pub rest_start_at: Option<i64>,
    pub session_rest_ms: i64,
    pub saved_at: i64,
}

impl TimerSessionSnapshot {
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "subject_id": self.subject_id,
            "subject_title": self.subject_title,
            "segment_start_at": self.segment_start_at,
            "accumulated_ms": self.accumulated_ms,
            "is_resting": self.is_resting,
            "rest_start_at": self.rest_start_at,
            "session_rest_ms": self.session_rest_ms,
            "saved_at": self.saved_at,
        })
    }

    pub fn from_json(json: &Value) -> Self {
        Self {
            subject_id: json.get("subject_id").and_then(Value::as_i64).unwrap_or(-1),
            subject_title: json
                .get("subject_title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            segment_start_at: json.get("segment_start_at").and_then(Value::as_i64),
            accumulated_ms: json.get("accumulated_ms").and_then(Value::as_i64).unwrap_or(0),
            is_resting: json.get("is_resting").and_then(Value::as_bool).unwrap_or(false),
            rest_start_at: json.get("rest_start_at").and_then(Value::as_i64),
            session_rest_ms: json.get("session_rest_ms").and_then(Value::as_i64).unwrap_or(0),
            saved_at: json
                .get("saved_at")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| Local::now().timestamp_millis()),
        }
    }

    pub fn segment_start(&self) -> Option<DateTime<Local>> {
        self.segment_start_at
            .and_then(DateTime::from_timestamp_millis)
            .map(|d| d.with_timezone(&Local))
    }

    pub fn rest_start(&self) -> Option<DateTime<Local>> {
        self.rest_start_at
            .and_then(DateTime::from_timestamp_millis)
            .map(|d| d.with_timezone(&Local))
    }

    pub fn saved_at_dt(&self) -> Option<DateTime<Local>> {
        DateTime::from_timestamp_millis(self.saved_at).map(|d| d.with_timezone(&Local))
    }
}

pub struct TimerSessionStore {
    // Store is behind a mutex in the app; the store handle itself is passed per call.
}

impl TimerSessionStore {
    pub fn new() -> Self {
        Self {}
    }

    pub fn save(store: &std::sync::Arc<std::sync::Mutex<Store>>, snapshot: &TimerSessionSnapshot) {
        let Ok(mut s) = store.lock() else { return; };
        let encoded = serde_json::to_string(&snapshot.to_json()).unwrap_or_default();
        s.prefs_set_string(STORAGE_KEY, &encoded);
        s.flush();
    }

    /// Loads the snapshot, dropping anything older than 24h (a stale snapshot
    /// must not resurrect a session and inflate reported study time).
    pub fn load(store: &std::sync::Arc<std::sync::Mutex<Store>>) -> Option<TimerSessionSnapshot> {
        let Ok(s) = store.lock() else { return None; };
        let raw = s.prefs_get_string(STORAGE_KEY)?;
        if raw.is_empty() {
            return None;
        }
        drop(s);
        let decoded = serde_json::from_str::<Value>(&raw).ok()?;
        if !decoded.is_object() {
            return None;
        }
        let snapshot = TimerSessionSnapshot::from_json(&decoded);
        let saved_at = snapshot.saved_at_dt()?;
        let age = Local::now().signed_duration_since(saved_at);
        if age < Duration::zero() || age > Duration::hours(24) {
            Self::clear(store);
            return None;
        }
        Some(snapshot)
    }

    pub fn clear(store: &std::sync::Arc<std::sync::Mutex<Store>>) {
        let Ok(mut s) = store.lock() else { return; };
        s.prefs_remove(STORAGE_KEY);
        s.flush();
    }
}
