// SPDX-License-Identifier: MIT

use chrono_tz::Tz;
use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, CosmicConfigEntry, PartialEq, Serialize, Deserialize)]
#[version = 2]
pub struct Config {
    /// Saved world clocks (timezone names)
    pub world_clocks: Vec<SavedClock>,
    /// Saved alarms
    pub alarms: Vec<SavedAlarm>,
    /// Saved timers
    pub timers: Vec<SavedTimer>,
    /// Saved pomodoro timers
    pub pomodoros: Vec<SavedPomodoro>,
    /// Pomodoro default durations
    pub pomodoro_defaults: PomodoroDefaults,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedClock {
    pub timezone: Tz,
    pub city_name: String,
    pub is_local: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedAlarm {
    pub hour: u8,
    pub minute: u8,
    pub label: String,
    pub is_enabled: bool,
    pub repeat_mode: SavedRepeatMode,
    pub sound: String,
    pub snooze_minutes: u8,
    pub ring_minutes: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SavedRepeatMode {
    Once,
    EveryDay,
    Custom(Vec<String>), // Day short names: "Mon", "Tue", etc.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTimer {
    pub label: String,
    pub duration_secs: u64,
    pub repeat_enabled: bool,
    pub repeat_count: u32,
    pub sound: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedPomodoro {
    pub label: String,
    pub work_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub sound: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PomodoroDefaults {
    pub work_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
}

impl Default for PomodoroDefaults {
    fn default() -> Self {
        Self {
            work_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
        }
    }
}
