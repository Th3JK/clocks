// SPDX-License-Identifier: MIT

use chrono_tz::Tz;
use cosmic_config::{CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, CosmicConfigEntry, PartialEq, Serialize, Deserialize)]
#[version = 5]
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
    /// 24-hour, 12-hour, or follow the desktop.
    pub time_format: crate::time_format::TimeFormat,
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
    /// Automatically clear stopwatch history after session ends
    #[serde(default)]
    pub auto_clear_stopwatch_history: bool,
    /// Saved stopwatch history records
    #[serde(default)]
    pub stopwatch_history: Vec<SavedStopwatchRecord>,
    /// Date-indexed pomodoro focus statistics (global, across all timers)
    #[serde(default)]
    pub pomodoro_stats: Vec<PomodoroDayStat>,
    /// Chess clock configuration
    #[serde(default)]
    pub chess: SavedChessConfig,
    /// Saved workout (HIIT/Tabata) presets
    #[serde(default)]
    pub workouts: Vec<SavedWorkout>,
    /// Saved countdown events
    #[serde(default)]
    pub countdown_events: Vec<SavedCountdownEvent>,
    /// Sidebar page order, by stable page key. Empty means "never customised",
    /// which restores the built-in order — and lets a page added in a later
    /// release appear rather than being treated as hidden.
    #[serde(default)]
    pub nav_order: Vec<String>,
    /// Page keys hidden from the sidebar.
    #[serde(default)]
    pub nav_hidden: Vec<String>,
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
            time_format: crate::time_format::TimeFormat::System,
            confirm_delete_alarm: true,
            confirm_delete_timer: true,
            confirm_delete_world_clock: true,
            confirm_delete_pomodoro: true,
            confirm_clear_stopwatch: true,
            auto_sort_alarms: false,
            auto_sort_world_clocks: false,
            auto_clear_stopwatch_history: false,
            stopwatch_history: Vec::new(),
            pomodoro_stats: Vec::new(),
            chess: SavedChessConfig::default(),
            workouts: Vec::new(),
            countdown_events: Vec::new(),
            nav_order: Vec::new(),
            nav_hidden: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedCountdownEvent {
    /// Stable identity. The daemon records delivered reminders against it, so
    /// positional ids would re-point them on reorder.
    pub id: u32,
    pub label: String,
    pub target: chrono::DateTime<chrono::Local>,
    pub yearly: bool,
    pub sound: String,
    /// Reminder offsets, stored by name so the set can grow without breaking
    /// existing configs — unknown names are dropped on load.
    pub reminders: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedWorkout {
    pub label: String,
    pub sound: String,
    pub blocks: Vec<SavedBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedBlock {
    pub repeat: u32,
    pub steps: Vec<SavedStep>,
    pub skip_last_recovery: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedStep {
    pub label: String,
    pub secs: u32,
    pub kind: SavedStepKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SavedStepKind {
    Prep,
    Effort,
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedChessConfig {
    pub base_minutes: u32,
    pub increment_secs: u32,
}

impl Default for SavedChessConfig {
    fn default() -> Self {
        Self {
            base_minutes: 5,
            increment_secs: 0,
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
    /// Stable identity, independent of position in the list.
    ///
    /// Ids used to be derived from list position on load, so reordering the
    /// alarms renumbered them -- and a pending snooze, which references its
    /// alarm by id, would silently reattach to a different one. Auto-sort made
    /// that happen during an ordinary save. `0` means a config written before
    /// this field existed; `restore_alarms` assigns those positionally once.
    #[serde(default)]
    pub id: u32,
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
    /// Stable identity, independent of list position. The daemon references a
    /// running timer by id, so deriving it from position would re-point a live
    /// run at a different timer the moment the list is reordered.
    /// `0` means a config written before this field existed.
    #[serde(default)]
    pub id: u32,
    pub label: String,
    pub duration_secs: u64,
    pub repeat_enabled: bool,
    pub repeat_count: u32,
    pub sound: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedPomodoro {
    /// Stable identity. The daemon references a running session by id, so
    /// deriving it from list position would re-point a live run at a different
    /// pomodoro as soon as the list is reordered.
    pub id: u32,
    pub label: String,
    pub work_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub sound: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PomodoroDayStat {
    /// Day in `YYYY-MM-DD` format.
    pub date: String,
    pub focus_secs: u64,
    pub sessions: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PomodoroDefaults {
    pub work_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedStopwatchRecord {
    pub label: String,
    pub total_elapsed_ms: u64,
    pub laps: Vec<SavedLap>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedLap {
    pub lap_time_ms: u64,
    pub delta_ms: i64,
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
