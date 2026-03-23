// SPDX-License-Identifier: MIT
//
// Stopwatch data types: lap entries, history records, and state.

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct LapEntry {
    pub id: u32,
    pub lap_time: Duration,
    pub delta: i64,
    pub is_fastest: bool,
    pub is_slowest: bool,
}

#[derive(Debug, Clone)]
pub struct StopwatchRecord {
    pub id: u32,
    pub label: String,
    pub total_elapsed: Duration,
    pub laps: Vec<LapEntry>,
}

pub struct StopwatchState {
    pub elapsed: Duration,
    pub is_running: bool,
    pub start_instant: Option<Instant>,
    pub accumulated: Duration,
    pub laps: Vec<LapEntry>,
    pub lap_start: Duration,
    pub next_lap_id: u32,
    // History
    pub history: Vec<StopwatchRecord>,
    pub next_history_id: u32,
    pub current_label: String,
    pub current_session_id: Option<u32>,
}

impl Default for StopwatchState {
    fn default() -> Self {
        Self {
            elapsed: Duration::ZERO,
            is_running: false,
            start_instant: None,
            accumulated: Duration::ZERO,
            laps: Vec::new(),
            lap_start: Duration::ZERO,
            next_lap_id: 1,
            history: Vec::new(),
            next_history_id: 1,
            current_label: String::new(),
            current_session_id: None,
        }
    }
}

impl StopwatchState {
    pub(super) fn current_elapsed(&self) -> Duration {
        if let Some(start) = self.start_instant {
            self.accumulated + start.elapsed()
        } else {
            self.accumulated
        }
    }
}
