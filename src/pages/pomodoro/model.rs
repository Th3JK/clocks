// SPDX-License-Identifier: MIT
//
// Pomodoro data types: session types, timer state, and defaults.

use crate::fl;
use chrono::Datelike;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionType {
    Work,
    ShortBreak,
    LongBreak,
}

impl SessionType {
    pub fn display_name(&self) -> String {
        match self {
            SessionType::Work => fl!("session-work"),
            SessionType::ShortBreak => fl!("session-short-break"),
            SessionType::LongBreak => fl!("session-long-break"),
        }
    }
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[derive(Debug, Clone)]
pub struct PomodoroTimer {
    pub id: u32,
    pub label: String,
    pub work_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub session_number: u32,
    pub session_type: SessionType,
    pub remaining: Duration,
    pub is_running: bool,
    pub start_instant: Option<Instant>,
    pub started_remaining: Duration,
    pub completed_work_sessions: u32,
    pub total_focused_secs: u64,
    pub target_sessions: u32,
    pub sound: String,
}

impl PomodoroTimer {
    pub fn from_config(id: u32, label: String, work: u32, short_break: u32, long_break: u32) -> Self {
        Self::new(id, label, work, short_break, long_break)
    }

    pub(super) fn new(id: u32, label: String, work: u32, short_break: u32, long_break: u32) -> Self {
        let work_dur = Duration::from_secs(work as u64 * 60);
        Self {
            id,
            label,
            work_minutes: work,
            short_break_minutes: short_break,
            long_break_minutes: long_break,
            session_number: 1,
            session_type: SessionType::Work,
            remaining: work_dur,
            is_running: false,
            start_instant: None,
            started_remaining: work_dur,
            completed_work_sessions: 0,
            total_focused_secs: 0,
            target_sessions: 8,
            sound: "Bell".to_string(),
        }
    }

    pub(super) fn work_duration(&self) -> Duration {
        Duration::from_secs(self.work_minutes as u64 * 60)
    }

    /// Full duration of the session currently in progress (for the progress ring).
    pub(super) fn session_total(&self) -> Duration {
        match self.session_type {
            SessionType::Work => self.work_duration(),
            SessionType::ShortBreak => self.short_break_duration(),
            SessionType::LongBreak => self.long_break_duration(),
        }
    }

    /// Whether this timer has progressed beyond its pristine, never-started state.
    pub(super) fn has_started(&self) -> bool {
        self.is_running
            || self.remaining < self.session_total()
            || self.session_number > 1
            || self.completed_work_sessions > 0
    }

    pub(super) fn short_break_duration(&self) -> Duration {
        Duration::from_secs(self.short_break_minutes as u64 * 60)
    }

    pub(super) fn long_break_duration(&self) -> Duration {
        Duration::from_secs(self.long_break_minutes as u64 * 60)
    }

    pub(super) fn advance_session(&mut self) {
        match self.session_type {
            SessionType::Work => {
                self.completed_work_sessions += 1;
                self.total_focused_secs += self.work_minutes as u64 * 60;
                if self.completed_work_sessions.is_multiple_of(4) {
                    self.session_type = SessionType::LongBreak;
                    self.remaining = self.long_break_duration();
                } else {
                    self.session_type = SessionType::ShortBreak;
                    self.remaining = self.short_break_duration();
                }
            }
            SessionType::ShortBreak | SessionType::LongBreak => {
                self.session_number += 1;
                self.session_type = SessionType::Work;
                self.remaining = self.work_duration();
            }
        }
        self.started_remaining = self.remaining;
        self.start_instant = Some(Instant::now());
    }
}

/// One day's aggregated focus statistics (global, across all pomodoro timers).
#[derive(Debug, Clone)]
pub struct DayStat {
    pub date: chrono::NaiveDate,
    pub focus_secs: u64,
    pub sessions: u32,
}

pub struct PomodoroState {
    pub timers: Vec<PomodoroTimer>,
    pub next_id: u32,
    // Settings defaults for new timers
    pub default_work_minutes: u32,
    pub default_short_break_minutes: u32,
    pub default_long_break_minutes: u32,
    // Editing state for new/existing timer
    pub edit_label: String,
    pub editing_id: Option<u32>,
    pub edit_work_minutes: u32,
    pub edit_short_break_minutes: u32,
    pub edit_long_break_minutes: u32,
    pub edit_sound: String,
    // Edit mode (reorder/delete)
    pub edit_mode: bool,
    pub dragging_index: Option<usize>,
    pub pre_drag_order: Vec<u32>,
    /// Pomodoro shown full-page in focus mode. Session-only, not persisted.
    pub focused_id: Option<u32>,
    // Global focus statistics, date-indexed (pruned to the last ~90 days)
    pub daily_stats: Vec<DayStat>,
}

impl Default for PomodoroState {
    fn default() -> Self {
        let mut state = Self {
            timers: Vec::new(),
            next_id: 1,
            default_work_minutes: 25,
            default_short_break_minutes: 5,
            default_long_break_minutes: 15,
            edit_label: String::new(),
            editing_id: None,
            edit_work_minutes: 25,
            edit_short_break_minutes: 5,
            edit_long_break_minutes: 15,
            edit_sound: "Bell".to_string(),
            edit_mode: false,
            dragging_index: None,
            pre_drag_order: Vec::new(),
            focused_id: None,
            daily_stats: Vec::new(),
        };
        // Create a default pomodoro timer
        state
            .timers
            .push(PomodoroTimer::new(0, "Pomodoro".to_string(), 25, 5, 15));
        state
    }
}

impl PomodoroState {
    pub fn is_running(&self) -> bool {
        self.timers.iter().any(|t| t.is_running)
    }

    /// Record a completed work session of `secs` into today's stats, then prune
    /// entries older than ~90 days.
    pub fn record_completed_work(&mut self, secs: u64) {
        let today = chrono::Local::now().date_naive();
        if let Some(entry) = self.daily_stats.iter_mut().find(|d| d.date == today) {
            entry.focus_secs += secs;
            entry.sessions += 1;
        } else {
            self.daily_stats.push(DayStat {
                date: today,
                focus_secs: secs,
                sessions: 1,
            });
        }
        let cutoff = today - chrono::Duration::days(90);
        self.daily_stats.retain(|d| d.date >= cutoff);
        self.daily_stats.sort_by_key(|d| d.date);
    }

    /// Total focus seconds recorded for today.
    pub fn focus_today(&self) -> u64 {
        let today = chrono::Local::now().date_naive();
        self.daily_stats
            .iter()
            .filter(|d| d.date == today)
            .map(|d| d.focus_secs)
            .sum()
    }

    /// Consecutive days (ending today, or yesterday if today is empty) that have
    /// at least one completed work session.
    pub fn current_streak(&self) -> u32 {
        use std::collections::HashSet;
        let days: HashSet<chrono::NaiveDate> = self
            .daily_stats
            .iter()
            .filter(|d| d.sessions > 0)
            .map(|d| d.date)
            .collect();
        let today = chrono::Local::now().date_naive();
        let mut cursor = if days.contains(&today) {
            today
        } else {
            today - chrono::Duration::days(1)
        };
        let mut streak = 0;
        while days.contains(&cursor) {
            streak += 1;
            cursor -= chrono::Duration::days(1);
        }
        streak
    }

    /// Focus seconds for each of the last 7 days (oldest first), with a one-letter
    /// weekday label for the chart.
    pub fn last_7_days(&self) -> Vec<(String, u64)> {
        let today = chrono::Local::now().date_naive();
        (0..7)
            .rev()
            .map(|offset| {
                let date = today - chrono::Duration::days(offset);
                let secs = self
                    .daily_stats
                    .iter()
                    .filter(|d| d.date == date)
                    .map(|d| d.focus_secs)
                    .sum();
                let label = match date.weekday() {
                    chrono::Weekday::Mon => "M",
                    chrono::Weekday::Tue => "T",
                    chrono::Weekday::Wed => "W",
                    chrono::Weekday::Thu => "T",
                    chrono::Weekday::Fri => "F",
                    chrono::Weekday::Sat => "S",
                    chrono::Weekday::Sun => "S",
                }
                .to_string();
                (label, secs)
            })
            .collect()
    }
}
