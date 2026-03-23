// SPDX-License-Identifier: MIT
//
// Timer data types: timer entries and state.

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct TimerEntry {
    pub id: u32,
    pub label: String,
    pub initial_duration: Duration,
    pub remaining: Duration,
    pub is_running: bool,
    pub start_instant: Option<Instant>,
    pub started_remaining: Duration,
    pub repeat_enabled: bool,
    pub repeat_count: u32, // 0 = infinite
    pub completed_count: u32,
    pub sound: String,
}

pub struct TimerState {
    pub timers: Vec<TimerEntry>,
    pub next_id: u32,
    // Timer editing state
    pub editing: bool,
    pub edit_id: Option<u32>, // None = new, Some(id) = editing existing
    pub edit_hours: u8,
    pub edit_minutes: u8,
    pub edit_seconds: u8,
    pub edit_label: String,
    pub edit_repeat: bool,
    pub edit_repeat_count: u32,
    pub edit_sound: String,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            timers: Vec::new(),
            next_id: 1,
            editing: false,
            edit_id: None,
            edit_hours: 0,
            edit_minutes: 5,
            edit_seconds: 0,
            edit_label: String::new(),
            edit_repeat: false,
            edit_repeat_count: 1,
            edit_sound: "Bell".to_string(),
        }
    }
}
