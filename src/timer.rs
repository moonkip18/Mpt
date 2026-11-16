//! Port of `lib/features/timer/timer_notifier.dart`.
//!
//! The Dart `TimerNotifier` is an async state machine driven by timers; here a
//! dedicated worker thread owns the controller, receives `TimerCmd` messages
//! from the UI, and runs the periodic polls (30s stats, 20s day rollover).

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use chrono::{DateTime, Local};
use serde_json::Value;

use crate::daily_stats::DailyStats;
use crate::localization::tr_msg;
use crate::models::SubjectModel;
use crate::repositories::offline_sync::OfflineSyncRepository;
use crate::repositories::timer::{TimerApi, TimerStartResult};
use crate::settings::SettingsState;
use crate::state::{Shared, TimerState};
use crate::timer_session_store::{TimerSessionSnapshot, TimerSessionStore};
use crate::utils::study_date::get_study_date_string;

const OFFLINE_NOTICE: &str = "Saved offline — will sync to YPT automatically.";
const START_EMPTY_NOTICE: &str =
    "YPT did not accept the start. The timer still runs locally and will sync on stop.";

#[derive(Debug)]
pub enum TimerCmd {
    Start,
    Pause,
    Stop,
    SelectSubject(i64),
    CreateSubject(String),
    RefreshStats,
    ApplySettings { day_reset_hour: i64, language: String, timezone: String },
    ClearNotice,
    ProbeRemote,
    Shutdown,
}

pub struct TimerDeps {
    pub timer: Arc<dyn TimerApi>,
    pub subjects: Arc<dyn crate::repositories::subject::SubjectsApi>,
    pub offline: Arc<OfflineSyncRepository>,
    pub session_store: Arc<Mutex<crate::store::Store>>,
}

/// Spawn the timer worker; returns the command sender for the UI.
pub fn spawn_timer_thread(
    shared: Arc<Mutex<Shared>>,
    deps: Arc<TimerDeps>,
    settings: SettingsState,
    clock: Option<fn() -> DateTime<Local>>,
) -> Sender<TimerCmd> {
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_tx = tx.clone();
    std::thread::spawn(move || {
        let mut ctrl = TimerController::new(shared, deps, settings, clock, worker_tx);
        ctrl.bootstrap();
        ctrl.run(rx);
    });
    tx
}

struct TimerController {
    shared: Arc<Mutex<Shared>>,
    deps: Arc<TimerDeps>,
    settings: SettingsState,
    clock: fn() -> DateTime<Local>,
    tx: Sender<TimerCmd>,
    syncing: bool,
    booted: bool,
}

impl TimerController {
    fn new(
        shared: Arc<Mutex<Shared>>,
        deps: Arc<TimerDeps>,
        settings: SettingsState,
        clock: Option<fn() -> DateTime<Local>>,
        tx: Sender<TimerCmd>,
    ) -> Self {
        Self {
            shared,
            deps,
            settings,
            clock: clock.unwrap_or(Local::now as fn() -> DateTime<Local>),
            tx,
            syncing: false,
            booted: false,
        }
    }

    fn now(&self) -> DateTime<Local> {
        (self.clock)()
    }

    fn study_date(&self) -> String {
        get_study_date_string(self.now(), self.settings.day_reset_hour as u32)
    }

    fn with_state<F: FnOnce(&mut TimerState)>(&self, f: F) {
        let mut guard = self.shared.lock().unwrap();
        f(&mut guard.timer);
    }

    // ------------------------------------------------------------- bootstrap

    fn bootstrap(&mut self) {
        self.load_subjects();
        self.refresh_stats();
        if let Some(snapshot) = TimerSessionStore::load(&self.deps.session_store) {
            self.restore(snapshot);
        } else {
            self.probe_remote_activity();
        }
        self.booted = true;
        self.shared.lock().unwrap().timer_booted = true;
    }

    fn run(&mut self, rx: Receiver<TimerCmd>) {
        let mut next_stats = Instant::now() + StdDuration::from_secs(30);
        let mut next_day = Instant::now() + StdDuration::from_secs(20);
        let mut next_remote = Instant::now() + StdDuration::from_secs(60);
        loop {
            match rx.recv_timeout(StdDuration::from_millis(250)) {
                Ok(TimerCmd::Shutdown) => break,
                Ok(cmd) => self.handle(cmd),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
            self.tick();
            let now = Instant::now();
            // Server truth is polled, never inferred from clock drift.
            if now >= next_stats {
                let running = self.shared.lock().unwrap().timer.is_running;
                if !running && self.booted {
                    self.refresh_stats();
                }
                let interval = if self.shared.lock().unwrap().timer.remote_active {
                    StdDuration::from_secs(20)
                } else {
                    StdDuration::from_secs(30)
                };
                next_stats = Instant::now() + interval;
            }
            if now >= next_day {
                self.check_day_rollover();
                next_day = Instant::now() + StdDuration::from_secs(20);
            }
            // Is the phone still studying? Clear the badge first so a session
            // that ended does not leave it on screen forever.
            if now >= next_remote {
                let running = self.shared.lock().unwrap().timer.is_running;
                if !running {
                    self.with_state(|s| s.remote_active = false);
                    self.probe_remote_activity();
                }
                next_remote = Instant::now() + StdDuration::from_secs(60);
            }
        }
    }

    /// Port of `_tick`: folds the live segment (or rest) into the elapsed
    /// counters so the shared state the UI reads always matches the clock.
    fn tick(&self) {
        let now = self.now();
        let mut guard = self.shared.lock().unwrap();
        let t = &mut guard.timer;
        if t.is_resting {
            if let Some(rest_start) = t.rest_start_at {
                let delta = (now - rest_start).num_milliseconds();
                if delta > 0 {
                    t.session_rest_ms = t.accumulated_rest_ms + delta;
                }
            }
            return;
        }
        if !t.is_running {
            return;
        }
        let Some(segment_start) = t.segment_start_at else { return; };
        let delta = (now - segment_start).num_milliseconds();
        if delta < 0 {
            return;
        }
        t.session_elapsed_ms = t.accumulated_ms + delta;
    }

    fn handle(&mut self, cmd: TimerCmd) {
        match cmd {
            TimerCmd::Start => self.start(),
            TimerCmd::Pause => self.pause(),
            TimerCmd::Stop => self.stop(),
            TimerCmd::SelectSubject(id) => self.select_subject(id),
            TimerCmd::CreateSubject(title) => self.create_subject(title),
            TimerCmd::RefreshStats => self.refresh_stats(),
            TimerCmd::ApplySettings { day_reset_hour, language, timezone } => {
                self.apply_settings(day_reset_hour, language, timezone)
            }
            TimerCmd::ClearNotice => self.with_state(|s| s.clear_notice()),
            TimerCmd::ProbeRemote => self.probe_remote_activity(),
            TimerCmd::Shutdown => {}
        }
    }

    // -------------------------------------------------------------- subjects

    /// Create a subject on YPT, reload the list and select it. Runs on the
    /// timer thread; the UI only sends the command and reads the notice.
    fn create_subject(&self, title: String) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        // A non-empty sync queue means the last network calls failed; creating
        // would just pile another failed request onto it.
        if self.shared.lock().unwrap().offline.has_pending() {
            self.with_state(|t| t.notice = Some("You look offline — the subject needs a connection.".into()));
            return;
        }
        let colors = crate::utils::color::DEFAULT_SUBJECT_COLORS;
        let pick = rand::seq::SliceRandom::choose(colors, &mut rand::thread_rng())
            .copied()
            .unwrap_or(0xFFA78BFA) as i64;
        match self.deps.subjects.create_subject(title, pick) {
            Ok(created) => {
                self.load_subjects();
                self.select_subject(created.id);
                self.refresh_stats();
            }
            Err(e) => {
                let msg = match e {
                    crate::api::ApiError::Decode(m) => m,
                    other => tr_msg("subject_create_failed", None, Some("Falha ao criar matéria")) + &format!(" ({other})"),
                };
                self.with_state(|t| t.notice = Some(msg));
            }
        }
    }

    fn load_subjects(&self) {
        let result = self.deps.subjects.fetch_subjects_data(
            &self.settings.selected_language,
            &self.settings.timezone,
            crate::constants::DEFAULT_VERSION as i64,
        );
        if result.subjects.is_empty() {
            return; // Offline: keep whatever the cache returned last launch.
        }
        let mut subjects = result.subjects;
        subjects.sort_by_key(|s| s.order);
        let mut guard = self.shared.lock().unwrap();
        let current = guard.timer.current_subject.clone();
        let selected = Self::resolve_subject(current, &subjects);
        guard.timer.subjects = subjects;
        if let Some(sel) = selected {
            guard.timer.current_subject = Some(sel);
        }
    }

    fn resolve_subject(
        current: Option<SubjectModel>,
        subjects: &[SubjectModel],
    ) -> Option<SubjectModel> {
        if subjects.is_empty() {
            return None;
        }
        if let Some(current) = current {
            for s in subjects {
                if s.id == current.id {
                    return Some(s.clone());
                }
            }
        }
        subjects.first().cloned()
    }

    fn select_subject(&self, id: i64) {
        let mut guard = self.shared.lock().unwrap();
        if guard.timer.is_running || guard.timer.is_resting {
            return;
        }
        if let Some(subject) = guard.timer.subjects.iter().find(|s| s.id == id) {
            guard.timer.current_subject = Some(subject.clone());
        }
    }

    // ----------------------------------------------------------------- stats

    fn refresh_stats(&self) {
        let date = self.study_date();
        let dl = match self.deps.timer.fetch_daily_log_dl(None) {
            Ok(dl) => dl,
            Err(_) => return, // Keep the previous numbers instead of flashing zeros.
        };
        let mut guard = self.shared.lock().unwrap();
        let known = guard.timer.subjects.clone();
        guard.timer.stats = DailyStats::from_daily_log(
            dl.as_ref(),
            &date,
            &known,
        );
        guard.timer.last_study_date = date;
    }

    /// Detects an open session started on another device so the UI can say so.
    /// Read-only on purpose: mirroring a remote session would double count.
    fn probe_remote_activity(&self) {
        let Ok(Some(dl)) = self.deps.timer.fetch_daily_log_dl(None) else { return; };
        let Some(sessions) = dl.get("ls").and_then(Value::as_array) else { return; };
        let Some(last) = sessions.last() else { return; };
        let Some(start_raw) = last.get("st").and_then(Value::as_str) else { return; };
        let Ok(start) = DateTime::parse_from_rfc3339(start_raw) else { return; };
        let ms = last.get("sm").and_then(Value::as_i64).unwrap_or(0);
        if ms <= 0 {
            return;
        }
        let start = start.with_timezone(&Local);
        let end = start + chrono::Duration::milliseconds(ms);
        let still_open = (self.now() - end).num_seconds().abs() < 45;
        if still_open {
            let mut guard = self.shared.lock().unwrap();
            guard.timer.remote_active = true;
        }
    }

    fn check_day_rollover(&self) {
        let current = self.study_date();
        let mut guard = self.shared.lock().unwrap();
        if guard.timer.last_study_date.is_empty() || guard.timer.last_study_date == current {
            return;
        }
        guard.timer.last_study_date = current.clone();
        guard.timer.stats = DailyStats::empty(&current);
        drop(guard);
        self.refresh_stats();
        self.load_subjects();
    }

    // -------------------------------------------------------------- session

    fn restore(&self, snapshot: TimerSessionSnapshot) {
        let now = self.now();
        let subject = {
            let guard = self.shared.lock().unwrap();
            Self::find_subject(&guard.timer.subjects, snapshot.subject_id, &snapshot.subject_title)
                .or_else(|| guard.timer.current_subject.clone())
        };

        let mut elapsed = snapshot.accumulated_ms;
        if !snapshot.is_resting {
            if let Some(start) = snapshot.segment_start() {
                let delta = (now - start).num_milliseconds();
                if delta > 0 {
                    elapsed += delta;
                }
            }
        }
        let mut rest = snapshot.session_rest_ms;
        if snapshot.is_resting {
            if let Some(start) = snapshot.rest_start() {
                let delta = (now - start).num_milliseconds();
                if delta > 0 {
                    rest += delta;
                }
            }
        }

        let mut guard = self.shared.lock().unwrap();
        let t = &mut guard.timer;
        t.is_running = !snapshot.is_resting;
        t.is_resting = snapshot.is_resting;
        t.segment_start_at = if snapshot.is_resting { None } else { snapshot.segment_start() };
        t.accumulated_ms = snapshot.accumulated_ms;
        t.session_elapsed_ms = elapsed;
        t.session_rest_ms = rest;
        t.rest_start_at = if snapshot.is_resting { snapshot.rest_start() } else { None };
        if let Some(subject) = subject {
            t.current_subject = Some(subject);
        }
        t.segment_base_ms = snapshot.accumulated_ms;
        t.last_synced_ms = snapshot.accumulated_ms;
        t.accumulated_rest_ms = snapshot.session_rest_ms;
    }

    fn find_subject(
        subjects: &[SubjectModel],
        id: i64,
        title: &str,
    ) -> Option<SubjectModel> {
        subjects.iter().find(|s| s.id == id).cloned().or_else(|| {
            subjects.iter().find(|s| s.title == title).cloned()
        })
    }

    fn save_snapshot(&self) {
        let guard = self.shared.lock().unwrap();
        let t = &guard.timer;
        let Some(subject) = t.current_subject.as_ref() else { return; };
        if !t.has_session() {
            return;
        }
        let snapshot = TimerSessionSnapshot {
            subject_id: subject.id,
            subject_title: subject.title.clone(),
            segment_start_at: if t.is_running {
                t.segment_start_at.map(|d| d.timestamp_millis())
            } else {
                None
            },
            accumulated_ms: t.accumulated_ms,
            is_resting: t.is_resting,
            rest_start_at: if t.is_resting {
                t.rest_start_at.map(|d| d.timestamp_millis())
            } else {
                None
            },
            session_rest_ms: t.session_rest_ms,
            saved_at: self.now().timestamp_millis(),
        };
        drop(guard);
        TimerSessionStore::save(&self.deps.session_store, &snapshot);
    }

    fn clear_snapshot(&self) {
        TimerSessionStore::clear(&self.deps.session_store);
    }

    fn start(&mut self) {
        {
            let guard = self.shared.lock().unwrap();
            if guard.timer.is_running || guard.timer.busy {
                return;
            }
            if guard.timer.current_subject.is_none() {
                return;
            }
        }
        let now = self.now();
        let was_resting = self.shared.lock().unwrap().timer.is_resting;

        if was_resting {
            self.post_rest(now);
        }

        self.with_state(|s| {
            s.busy = true;
            s.notice = None;
        });

        let (subject_title, subject_id) = {
            let guard = self.shared.lock().unwrap();
            let subject = guard.timer.current_subject.clone().unwrap();
            (subject.title, subject.id)
        };

        // Mirrors `.timeout(8s, onTimeout: (ok:false, message:null))`: a timeout
        // yields no notice, an explicit refusal surfaces the server message.
        let failure: Option<String> = match self
            .deps
            .timer
            .start_study(&subject_title, subject_id, now)
        {
            TimerStartResult { ok: true, .. } => None,
            TimerStartResult { ok: false, message } => message,
        };

        let mut guard = self.shared.lock().unwrap();
        let t = &mut guard.timer;
        let session_elapsed = t.session_elapsed_ms;
        t.accumulated_rest_ms = 0;
        t.busy = false;
        t.is_running = true;
        t.is_resting = false;
        t.segment_start_at = Some(now);
        t.rest_start_at = None;
        t.session_rest_ms = 0;
        t.accumulated_ms = session_elapsed;
        t.segment_base_ms = session_elapsed;
        // A session of our own means this device is the active one.
        t.remote_active = false;
        t.notice = failure.map(|f| {
            if f.is_empty() {
                START_EMPTY_NOTICE.to_string()
            } else {
                format!("YPT: {f}")
            }
        });
        drop(guard);
        self.save_snapshot();
    }

    fn pause(&mut self) {
        {
            let guard = self.shared.lock().unwrap();
            if !guard.timer.is_running || guard.timer.busy {
                return;
            }
        }
        self.with_state(|s| s.busy = true);
        self.sync_segment();
        let now = self.now();

        let mut guard = self.shared.lock().unwrap();
        let t = &mut guard.timer;
        let elapsed_now = Self::computed_elapsed(t, now);
        t.accumulated_rest_ms = 0;
        t.busy = false;
        t.is_running = false;
        t.is_resting = true;
        t.segment_start_at = None;
        t.rest_start_at = Some(now);
        t.session_rest_ms = 0;
        t.session_elapsed_ms = elapsed_now;
        t.accumulated_ms = elapsed_now;
        drop(guard);
        self.save_snapshot();
    }

    fn stop(&mut self) {
        {
            let guard = self.shared.lock().unwrap();
            if guard.timer.busy {
                return;
            }
        }
        self.with_state(|s| s.busy = true);
        let (running, resting) = {
            let guard = self.shared.lock().unwrap();
            (guard.timer.is_running, guard.timer.is_resting)
        };
        if running {
            self.sync_segment();
        } else if resting {
            self.post_rest(self.now());
        }

        let mut guard = self.shared.lock().unwrap();
        let t = &mut guard.timer;
        t.accumulated_rest_ms = 0;
        t.busy = false;
        t.is_running = false;
        t.is_resting = false;
        t.segment_start_at = None;
        t.rest_start_at = None;
        t.session_rest_ms = 0;
        t.session_elapsed_ms = 0;
        t.accumulated_ms = 0;
        t.last_synced_ms = 0;
        t.segment_base_ms = 0;
        t.remote_active = false;
        drop(guard);
        self.clear_snapshot();
        self.refresh_stats();
    }

    fn computed_elapsed(t: &TimerState, now: DateTime<Local>) -> i64 {
        if t.is_running {
            if let Some(start) = t.segment_start_at {
                let delta = (now - start).num_milliseconds();
                if delta > 0 {
                    return t.accumulated_ms + delta;
                }
            }
        }
        t.session_elapsed_ms.max(t.accumulated_ms)
    }

    /// Pushes the segment that has not reached YPT yet. On failure the segment
    /// is queued and marked "known" locally, so the queue is the single owner of
    /// that time and the UI never counts it twice.
    fn sync_segment(&mut self) {
        if self.syncing {
            return;
        }
        let now = self.now();
        let (start, elapsed, segment_ms, subject) = {
            let guard = self.shared.lock().unwrap();
            let t = &guard.timer;
            let start = match t.segment_start_at {
                Some(s) => s,
                None => return,
            };
            let subject = match t.current_subject.clone() {
                Some(s) => s,
                None => return,
            };
            let elapsed = Self::computed_elapsed(t, now);
            let segment_ms = elapsed - t.segment_base_ms;
            (start, elapsed, segment_ms, subject)
        };
        if segment_ms <= 0 {
            return;
        }

        self.syncing = true;
        let stop_at = start + chrono::Duration::milliseconds(segment_ms);
        let outcome = self.deps.timer.stop_study(
            &subject.title,
            subject.id,
            stop_at,
            segment_ms,
            start,
        );
        self.syncing = false;

        match outcome {
            Ok(Some(res)) => {
                let mut guard = self.shared.lock().unwrap();
                guard.timer.last_synced_ms = elapsed;
                let known = guard.timer.subjects.clone();
                let date = guard.timer.stats.date.clone();
                guard.timer.stats = Self::stats_from_response(&res, subject, segment_ms, date, &known);
            }
            _ => {
                // Network failure or rejection: queue it (server copy wins on replay).
                self.deps.offline.enqueue_session(
                    subject.id,
                    &subject.title,
                    start,
                    stop_at,
                    segment_ms,
                );
                self.shared.lock().unwrap().timer.last_synced_ms = elapsed;
                self.with_state(|s| s.notice = Some(OFFLINE_NOTICE.to_string()));
            }
        }
    }

    fn stats_from_response(
        res: &Value,
        subject: SubjectModel,
        segment_ms: i64,
        date: String,
        known: &[SubjectModel],
    ) -> DailyStats {
        if let Some(dl) = res.get("dl") {
            if dl.is_object() {
                return DailyStats::from_daily_log(Some(dl), &date, known);
            }
        }
        // No totals in the response: keep local truth and bump the row.
        let base = DailyStats::empty(&date);
        base.with_live_delta(Some(subject.id), Some(&subject.title), segment_ms)
    }

    fn post_rest(&self, until: DateTime<Local>) {
        let (start, rest_ms) = {
            let guard = self.shared.lock().unwrap();
            let t = &guard.timer;
            let start = match t.rest_start_at {
                Some(s) => s,
                None => return,
            };
            // _postRest reads restStartAt + the live sessionRestMs value.
            (start, t.session_rest_ms)
        };
        if rest_ms < 5000 {
            return;
        }
        let _ = self.deps.timer.record_rest(start, until, rest_ms);
    }

    fn apply_settings(&mut self, day_reset_hour: i64, language: String, timezone: String) {
        let reset_changed = self.settings.day_reset_hour != day_reset_hour;
        self.settings.day_reset_hour = day_reset_hour;
        if self.settings.selected_language != language || self.settings.timezone != timezone {
            self.settings.selected_language = language;
            self.settings.timezone = timezone;
            self.load_subjects();
        }
        if reset_changed {
            self.check_day_rollover();
        }
        let current = self.study_date();
        self.with_state(|s| s.last_study_date = current);
    }
}
