//! Port of `lib/data/repositories/offline_sync_repository.dart`.

use chrono::{DateTime, Local};
use serde_json::Value;

use crate::localization::tr_msg;
use crate::models::OfflineSessionModel;
use crate::repositories::timer::TimerApi;
use crate::store::Store;

use std::sync::{Arc, Mutex};

const STORAGE_KEY: &str = "desky_offline_study_sessions";

#[derive(Default)]
pub struct SyncResult {
    pub total_synced: i64,
    pub total_failed: i64,
    pub remaining: i64,
}

pub struct OfflineSyncRepository {
    timer_repository: Arc<dyn TimerApi>,
    store: Arc<Mutex<Store>>,
}

impl OfflineSyncRepository {
    pub fn new(timer_repository: Arc<dyn TimerApi>, store: Arc<Mutex<Store>>) -> Self {
        Self { timer_repository, store }
    }

    pub fn get_pending_sessions(&self) -> Vec<OfflineSessionModel> {
        let Ok(store) = self.store.lock() else { return vec![]; };
        store
            .prefs_get_string_list(STORAGE_KEY)
            .iter()
            .filter_map(|item| serde_json::from_str::<Value>(item).ok())
            .map(|v| OfflineSessionModel::from_json(&v))
            .collect()
    }

    pub fn enqueue_session(
        &self,
        subject_id: i64,
        subject_title: &str,
        start_at: DateTime<Local>,
        stop_at: DateTime<Local>,
        study_ms: i64,
    ) {
        let new_session = OfflineSessionModel {
            id: format!("{}_{}", Local::now().timestamp_millis(), subject_id),
            subject_id,
            subject_title: subject_title.to_string(),
            start_at: start_at.timestamp_millis(),
            stop_at: stop_at.timestamp_millis(),
            study_ms,
            created_at: Local::now().timestamp_millis(),
            retry_count: 0,
            last_error: None,
        };
        let mut current = self.get_pending_sessions();
        current.push(new_session);
        self.persist(&current);
    }

    pub fn remove_session(&self, id: &str) {
        let mut current = self.get_pending_sessions();
        current.retain(|s| s.id != id);
        self.persist(&current);
    }

    /// Replays the queue, skipping stops the server already has.
    pub fn sync_all_pending(&self) -> SyncResult {
        let pending = self.get_pending_sessions();
        if pending.is_empty() {
            return SyncResult::default();
        }

        let mut synced: i64 = 0;
        let mut failed: i64 = 0;
        let mut remaining_list: Vec<OfflineSessionModel> = Vec::new();

        for session in pending {
            let start_at = DateTime::from_timestamp_millis(session.start_at)
                .map(|d| d.with_timezone(&Local))
                .unwrap_or_else(Local::now);
            let stop_at = DateTime::from_timestamp_millis(session.stop_at)
                .map(|d| d.with_timezone(&Local))
                .unwrap_or_else(Local::now);

            // A queued stop can already exist server-side (response lost, or the
            // app died between request and queue write). Server copy wins.
            let already_there = self.timer_repository.has_recorded_session(
                &session.subject_title,
                start_at,
                chrono::Duration::minutes(3),
            );
            if already_there {
                synced += 1;
                continue;
            }

            match self.timer_repository.stop_study(
                &session.subject_title,
                session.subject_id,
                stop_at,
                session.study_ms,
                start_at,
            ) {
                Ok(Some(_)) => synced += 1,
                Ok(None) => {
                    failed += 1;
                    remaining_list.push(session.copy_with(
                        Some(session.retry_count + 1),
                        Some(Some(tr_msg(
                            "sync_server_rejected",
                            None,
                            Some("Servidor rejeitou sessão"),
                        ))),
                    ));
                }
                Err(e) => {
                    failed += 1;
                    remaining_list.push(session.copy_with(
                        Some(session.retry_count + 1),
                        Some(Some(e.to_string())),
                    ));
                }
            }
        }

        self.persist(&remaining_list);
        SyncResult {
            total_synced: synced,
            total_failed: failed,
            remaining: remaining_list.len() as i64,
        }
    }

    pub fn clear_queue(&self) {
        let Ok(mut store) = self.store.lock() else { return; };
        store.prefs_remove(STORAGE_KEY);
        store.flush();
    }

    /// Cheap authenticated probe used to end offline mode automatically:
    /// success means the internet (and YPT) is reachable again.
    pub fn ping(&self) -> bool {
        self.timer_repository
            .fetch_daily_log_dl(None)
            .is_ok()
    }

    fn persist(&self, sessions: &[OfflineSessionModel]) {
        let Ok(mut store) = self.store.lock() else { return; };
        let encoded: Vec<String> = sessions
            .iter()
            .map(|s| serde_json::to_string(&s.to_json()).unwrap_or_default())
            .collect();
        store.prefs_set_string_list(STORAGE_KEY, &encoded);
        store.flush();
    }
}
