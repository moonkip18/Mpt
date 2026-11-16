//! Shared application state — the Rust counterpart of the Riverpod notifiers
//! (`auth_notifier`, `timer_notifier`, `settings_notifier`, `offline_sync_notifier`,
//! `mini_player_notifier`), held behind one lock for both the UI thread and the
//! worker threads that talk to YPT.

use chrono::{DateTime, Local};

use crate::daily_stats::DailyStats;
use crate::models::{OfflineSessionModel, SubjectModel, UserModel};

#[derive(Clone, Debug, Default)]
pub struct AuthState {
    pub user: Option<UserModel>,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

impl AuthState {
    pub fn is_authenticated(&self) -> bool {
        self.user.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct TimerState {
    pub subjects: Vec<SubjectModel>,
    pub current_subject: Option<SubjectModel>,

    /// Server truth for the current study day.
    pub stats: DailyStats,

    pub is_running: bool,
    pub is_resting: bool,

    /// Instant the current running segment began; None while resting/idle.
    pub segment_start_at: Option<DateTime<Local>>,
    pub accumulated_ms: i64,
    pub session_elapsed_ms: i64,

    pub session_rest_ms: i64,
    pub rest_start_at: Option<DateTime<Local>>,

    /// Elapsed milliseconds already stored on YPT for this session.
    pub last_synced_ms: i64,

    /// Session elapsed when the current segment began. YPT receives the segment
    /// duration, not the whole session, so a paused-and-resumed session is not
    /// recorded twice.
    pub segment_base_ms: i64,

    pub last_study_date: String,
    pub busy: bool,
    pub notice: Option<String>,

    /// Another device has an open session, so totals may move without us.
    pub remote_active: bool,

    /// Rest milliseconds accumulated before the current rest segment.
    pub accumulated_rest_ms: i64,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            subjects: Vec::new(),
            current_subject: None,
            stats: DailyStats::default(),
            is_running: false,
            is_resting: false,
            segment_start_at: None,
            accumulated_ms: 0,
            session_elapsed_ms: 0,
            session_rest_ms: 0,
            rest_start_at: None,
            last_synced_ms: 0,
            segment_base_ms: 0,
            last_study_date: String::new(),
            busy: false,
            notice: None,
            remote_active: false,
            accumulated_rest_ms: 0,
        }
    }
}

impl TimerState {
    pub fn has_session(&self) -> bool {
        self.is_running || self.is_resting || self.session_elapsed_ms > 0
    }

    /// Time in this session that YPT does not know about yet.
    pub fn live_delta_ms(&self) -> i64 {
        let delta = self.session_elapsed_ms - self.last_synced_ms;
        if delta > 0 { delta } else { 0 }
    }

    /// Today's total as the user should see it right now.
    pub fn display_total_ms(&self) -> i64 {
        self.stats.total_ms + self.live_delta_ms()
    }

    /// Per-subject totals with the unsynced remainder folded into the current row.
    pub fn display_stats(&self) -> DailyStats {
        self.stats.with_live_delta(
            self.current_subject.as_ref().map(|s| s.id),
            self.current_subject.as_ref().map(|s| s.title.as_str()),
            self.live_delta_ms(),
        )
    }

    pub fn clear_notice(&mut self) {
        self.notice = None;
    }
}

#[derive(Clone, Debug, Default)]
pub struct OfflineSyncState {
    pub pending_sessions: Vec<OfflineSessionModel>,
    pub is_syncing: bool,
    pub last_sync_message: Option<String>,
}

impl OfflineSyncState {
    pub fn pending_count(&self) -> usize {
        self.pending_sessions.len()
    }

    pub fn has_pending(&self) -> bool {
        !self.pending_sessions.is_empty()
    }
}

/// Window-level UI flags (mini player, popups, close-in-progress).
#[derive(Clone, Debug)]
pub struct UiFlags {
    pub is_mini: bool,
    pub all_subjects_open: bool,
    pub closing: bool,
}

impl Default for UiFlags {
    fn default() -> Self {
        Self {
            is_mini: false,
            all_subjects_open: false,
            closing: false,
        }
    }
}

/// Everything both threads read and write.
#[derive(Default)]
pub struct Shared {
    pub auth: AuthState,
    pub timer: TimerState,
    pub offline: OfflineSyncState,
    pub ui: UiFlags,
    /// Set once the timer bootstrap (subjects, stats, snapshot restore) ran.
    pub timer_booted: bool,
}
