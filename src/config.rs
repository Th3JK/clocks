// SPDX-License-Identifier: MIT

use chrono_tz::Tz;
use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, CosmicConfigEntry, PartialEq, Serialize, Deserialize)]
#[version = 3]
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
    /// Use 12-hour (AM/PM) time format instead of 24-hour
    pub use_12h: bool,
    /// Confirmation dialog settings (default: true = show confirmation)
    #[serde(default = "default_true")]
    pub confirm_delete_alarm: bool,
    #[serde(default = "default_true")]
    pub confirm_delete_timer: bool,
    #[serde(default = "default_true")]
    pub confirm_delete_world_clock: bool,
    #[serde(default = "default_true")]
    pub confirm_delete_pomodoro: bool,
    #[serde(default = "default_true")]
    pub confirm_clear_stopwatch: bool,
    /// Automatically sort alarms by time of activation
    #[serde(default)]
    pub auto_sort_alarms: bool,
    /// Automatically sort world clocks by timezone offset
    #[serde(default)]
    pub auto_sort_world_clocks: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            world_clocks: Vec::new(),
            alarms: Vec::new(),
            timers: Vec::new(),
            pomodoros: Vec::new(),
            pomodoro_defaults: PomodoroDefaults::default(),
            use_12h: false,
            confirm_delete_alarm: true,
            confirm_delete_timer: true,
            confirm_delete_world_clock: true,
            confirm_delete_pomodoro: true,
            confirm_clear_stopwatch: true,
            auto_sort_alarms: false,
            auto_sort_world_clocks: false,
        }
    }
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
