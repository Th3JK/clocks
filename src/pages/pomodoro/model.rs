// SPDX-License-Identifier: MIT
//
// Pomodoro data types: session types, timer state, and defaults.

use crate::fl;
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
}
