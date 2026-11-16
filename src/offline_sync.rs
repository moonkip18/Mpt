//! Port of `lib/features/timer/offline_sync_notifier.dart`.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use crate::localization::tr_msg;
use crate::repositories::offline_sync::{OfflineSyncRepository, SyncResult};
use crate::state::Shared;

#[derive(Debug)]
pub enum OfflineCmd {
    LoadPending,
    Enqueue {
        subject_id: i64,
        subject_title: String,
        start_at: i64,
        stop_at: i64,
        study_ms: i64,
    },
    SyncNow,
    /// User toggled offline mode from the strip.
    SetOffline(bool),
    /// Periodic: while offline, try the API and auto-exit the mode when the
    /// net is back, then replay the queue (previous days included).
    Probe,
    Shutdown,
}

/// How often the offline thread retries the API while offline. On success
/// the mode ends by itself and queued sessions go up.
const PROBE_INTERVAL: StdDuration = StdDuration::from_secs(30);

pub fn spawn_offline_thread(
    shared: Arc<Mutex<Shared>>,
    repo: Arc<OfflineSyncRepository>,
) -> Sender<OfflineCmd> {
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_tx = tx.clone();
    std::thread::spawn(move || {
        let mut ctrl = OfflineController::new(shared, repo, worker_tx);
        ctrl.load_pending();
        ctrl.run(rx);
    });
    tx
}

struct OfflineController {
    shared: Arc<Mutex<Shared>>,
    repo: Arc<OfflineSyncRepository>,
    tx: Sender<OfflineCmd>,
}

impl OfflineController {
    fn new(shared: Arc<Mutex<Shared>>, repo: Arc<OfflineSyncRepository>, tx: Sender<OfflineCmd>) -> Self {
        Self { shared, repo, tx }
    }

    fn run(&mut self, rx: Receiver<OfflineCmd>) {
        let mut next_probe = Instant::now() + PROBE_INTERVAL;
        loop {
            match rx.recv_timeout(PROBE_INTERVAL) {
                Ok(OfflineCmd::Shutdown) => break,
                Ok(cmd) => match cmd {
                    OfflineCmd::Shutdown => break,
                    OfflineCmd::LoadPending => self.load_pending(),
                    OfflineCmd::Enqueue { subject_id, subject_title, start_at, stop_at, study_ms } => {
                        let start = chrono::DateTime::from_timestamp_millis(start_at)
                            .map(|d| d.with_timezone(&chrono::Local))
                            .unwrap_or_else(chrono::Local::now);
                        let stop = chrono::DateTime::from_timestamp_millis(stop_at)
                            .map(|d| d.with_timezone(&chrono::Local))
                            .unwrap_or_else(chrono::Local::now);
                        self.repo.enqueue_session(subject_id, &subject_title, start, stop, study_ms);
                        self.load_pending();
                    }
                    OfflineCmd::SyncNow => self.sync_now(),
                    OfflineCmd::SetOffline(on) => self.set_offline(on),
                    OfflineCmd::Probe => self.probe(),
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if self.shared.lock().unwrap().offline.offline_mode && Instant::now() >= next_probe {
                self.probe();
                next_probe = Instant::now() + PROBE_INTERVAL;
            }
        }
    }

    fn set_offline(&self, on: bool) {
        {
            let mut guard = self.shared.lock().unwrap();
            guard.offline.offline_mode = on;
            if on {
                guard.offline.last_sync_message = None;
            }
        }
        if on {
            self.with_timer_state(|t| {
                t.notice = Some("Offline mode on — sessions are saved here and sent to YPT when the internet returns.".into());
            });
        } else {
            self.with_timer_state(|t| {
                t.notice = Some("Back online — sending saved sessions to YPT.".into());
            });
            self.sync_now();
            self.load_pending();
        }
    }

    fn with_timer_state<F: FnOnce(&mut crate::state::TimerState)>(&self, f: F) {
        let mut guard = self.shared.lock().unwrap();
        f(&mut guard.timer);
    }

    /// One cheap authenticated call. Success means the net is back: drop the
    /// inferred mode, replay the queue, refresh what the server knows.
    fn probe(&self) {
        if !self.repo.ping() {
            return;
        }
        let was_user_mode = self.shared.lock().unwrap().offline.offline_mode;
        if was_user_mode {
            let mut guard = self.shared.lock().unwrap();
            guard.offline.offline_mode = false;
        }
        self.load_pending();
        if self.shared.lock().unwrap().offline.has_pending() {
            self.sync_now();
        }
    }

    fn load_pending(&self) {
        let pending = self.repo.get_pending_sessions();
        let mut guard = self.shared.lock().unwrap();
        guard.offline.pending_sessions = pending;
    }

    fn sync_now(&self) {
        {
            let mut guard = self.shared.lock().unwrap();
            guard.offline.is_syncing = true;
        }

        // Network work happens off the UI lock (it already runs on this worker).
        let result: SyncResult = self.repo.sync_all_pending();
        let pending = self.repo.get_pending_sessions();

        let msg = if result.total_synced > 0 && result.total_failed == 0 {
            tr_msg(
                "sessions_synced_success",
                Some(vec![&result.total_synced.to_string()]),
                Some(&format!("{} sessions synced successfully!", result.total_synced)),
            )
        } else if result.total_failed > 0 {
            tr_msg(
                "sessions_sync_failed",
                Some(vec![&result.total_synced.to_string(), &result.total_failed.to_string()]),
                Some(&format!(
                    "{} synced, {} failed.",
                    result.total_synced, result.total_failed
                )),
            )
        } else {
            tr_msg("no_pending_sessions", None, None)
        };

        let mut guard = self.shared.lock().unwrap();
        guard.offline.pending_sessions = pending;
        guard.offline.is_syncing = false;
        guard.offline.last_sync_message = Some(msg);
    }
}

/// Convenience used by the auth flow: replay any queued stops right after login.
pub fn kick_sync(tx: &Sender<OfflineCmd>) {
    let _ = tx.send(OfflineCmd::SyncNow);
    let _ = tx.send(OfflineCmd::LoadPending);
    let _ = StdDuration::from_millis(0);
}
