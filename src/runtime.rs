// SPDX-License-Identifier: MIT

//! Runtime alarm state, owned by the daemon.
//!
//! Kept in a *separate* cosmic-config entry from [`crate::config::Config`],
//! stored through `Config::new_state`, because two processes writing one entry
//! cannot work here: `build_config_from_state` reconstructs the whole `Config`
//! from the GUI's in-memory state without reading what is on disk, and the GUI
//! saves on nearly every message. Anything the daemon wrote into that entry
//! would be gone by the next keystroke.
//!
//! So the ownership rule is one writer per entry:
//!
//! - `Config`       -- user definitions and settings. GUI writes, daemon reads.
//! - `RuntimeState` -- what is ringing, snoozed or spent. Daemon writes, GUI reads.
//!
//! The GUI observes this through `watch_state`, the same mechanism it already
//! uses for its own config.

use cosmic_config::{CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// An alarm currently ringing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RingingRecord {
    pub alarm_id: u32,
    pub label: String,
    pub sound: String,
    pub ring_secs: u64,
    pub snooze_minutes: u8,
    /// Wall clock, not `Instant`: this crosses a process boundary and has to
    /// survive the daemon being restarted mid-ring.
    pub started_at: chrono::DateTime<chrono::Local>,
}

/// An alarm waiting to re-ring after a snooze.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnoozeRecord {
    pub alarm_id: u32,
    pub label: String,
    pub sound: String,
    pub ring_minutes: u8,
    pub snooze_minutes: u8,
    pub retrigger_at: chrono::DateTime<chrono::Local>,
}

/// A timer the daemon is counting down.
///
/// Wall clock rather than `std::time::Instant`, which is monotonic and resets
/// per process: a timer that outlives the window cannot be described by one.
/// This is also what lets a running timer survive a restart, which it never did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimerRun {
    pub timer_id: u32,
    /// Expiry while running; `None` when paused.
    pub deadline: Option<chrono::DateTime<chrono::Local>>,
    /// Authoritative only while paused -- while running, the deadline is.
    pub remaining_secs: u64,
    /// Repeats already fired, counted against `repeat_count`.
    pub completed: u32,
}

/// Which phase of the pomodoro cycle a run is in.
///
/// Mirrors `pages::pomodoro::SessionType`, which is not serialisable and lives
/// behind the widget layer -- the same split as `SavedRepeatMode` and
/// `RepeatMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionKind {
    Work,
    ShortBreak,
    LongBreak,
}

/// A pomodoro the daemon is running.
///
/// Unlike a timer this does not simply end: on expiry it advances to the next
/// phase, which is why the cycle has to live here rather than in the GUI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PomodoroRun {
    pub timer_id: u32,
    pub session: SessionKind,
    pub session_number: u32,
    /// Expiry while running; `None` when paused.
    pub deadline: Option<chrono::DateTime<chrono::Local>>,
    /// Authoritative only while paused.
    pub remaining_secs: u64,
    pub completed_work_sessions: u32,
    /// Work seconds finished but not yet folded into `Config.pomodoro_stats`.
    ///
    /// Daily stats are a GUI-owned config entry, so the daemon cannot write
    /// them. It banks the total here and the GUI records it on next sight,
    /// clearing the counter -- late rather than never, and still one writer per
    /// entry.
    pub unrecorded_focus_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 1]
pub struct RuntimeState {
    #[serde(default)]
    pub ringing: Vec<RingingRecord>,

    #[serde(default)]
    pub snoozed: Vec<SnoozeRecord>,

    /// Ids of `RepeatMode::Once` alarms that have already fired.
    ///
    /// Firing a one-shot alarm used to clear `is_enabled` on the alarm itself,
    /// which is a *definition* field the GUI owns -- the daemon must not write
    /// there. The effective enabled state is therefore
    /// `alarm.is_enabled && !consumed_once.contains(&alarm.id)`, and re-enabling
    /// an alarm in the GUI clears its entry here.
    #[serde(default)]
    pub consumed_once: Vec<u32>,

    /// How far the scheduler has already checked.
    ///
    /// Replaces the old `last_triggered_minute`, which was a single global
    /// `(hour, minute)` tuple and so only suppressed a repeat *within* the same
    /// minute -- if the process was asleep across the minute an alarm was due,
    /// it was missed outright with no catch-up. A timestamp lets the scheduler
    /// fire anything that came due while it was not looking.
    #[serde(default)]
    pub checked_through: Option<chrono::DateTime<chrono::Local>>,

    /// Timers the daemon is counting down. Definitions stay in `Config`; this
    /// is only the running state, looked up by `timer_id`.
    #[serde(default)]
    pub timers: Vec<TimerRun>,

    /// Pomodoros the daemon is running.
    #[serde(default)]
    pub pomodoro: Vec<PomodoroRun>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            ringing: Vec::new(),
            snoozed: Vec::new(),
            consumed_once: Vec::new(),
            checked_through: None,
            timers: Vec::new(),
            pomodoro: Vec::new(),
        }
    }
}

impl RuntimeState {
    /// Open the state entry. Separate directory from the config entry, so the
    /// version here is independent of `Config`'s.
    pub fn context() -> Option<cosmic_config::Config> {
        cosmic_config::Config::new_state(crate::APP_ID, Self::VERSION).ok()
    }

    pub fn load(ctx: &cosmic_config::Config) -> Self {
        match Self::get_entry(ctx) {
            Ok(state) => state,
            Err((_errors, state)) => state,
        }
    }

    pub fn is_ringing(&self, alarm_id: u32) -> bool {
        self.ringing.iter().any(|r| r.alarm_id == alarm_id)
    }

    pub fn snooze_for(&self, alarm_id: u32) -> Option<&SnoozeRecord> {
        self.snoozed.iter().find(|s| s.alarm_id == alarm_id)
    }

    pub fn timer(&self, timer_id: u32) -> Option<&TimerRun> {
        self.timers.iter().find(|t| t.timer_id == timer_id)
    }

    pub fn pomodoro(&self, timer_id: u32) -> Option<&PomodoroRun> {
        self.pomodoro.iter().find(|p| p.timer_id == timer_id)
    }
}

impl TimerRun {
    /// Seconds left, from the deadline while running and from the stored
    /// remainder while paused. Saturates at zero rather than going negative.
    #[must_use]
    pub fn remaining_secs(&self, now: chrono::DateTime<chrono::Local>) -> u64 {
        match self.deadline {
            Some(deadline) => deadline
                .signed_duration_since(now)
                .num_seconds()
                .max(0)
                .unsigned_abs(),
            None => self.remaining_secs,
        }
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.deadline.is_some()
    }
}

impl SessionKind {
    /// Same strings the pomodoro page uses, so a notification and the window
    /// name the phase identically.
    #[must_use]
    pub fn display_name(self) -> String {
        match self {
            SessionKind::Work => crate::fl!("session-work"),
            SessionKind::ShortBreak => crate::fl!("session-short-break"),
            SessionKind::LongBreak => crate::fl!("session-long-break"),
        }
    }
}

impl PomodoroRun {
    #[must_use]
    pub fn remaining_secs(&self, now: chrono::DateTime<chrono::Local>) -> u64 {
        match self.deadline {
            Some(deadline) => deadline
                .signed_duration_since(now)
                .num_seconds()
                .max(0)
                .unsigned_abs(),
            None => self.remaining_secs,
        }
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.deadline.is_some()
    }
}
