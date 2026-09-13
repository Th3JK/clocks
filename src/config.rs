// SPDX-License-Identifier: MIT

use chrono_tz::Tz;
use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, CosmicConfigEntry, PartialEq, Serialize, Deserialize)]
#[version = 4]
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
    /// Legacy 12/24-hour flag, superseded by `time_format`.
    ///
    /// Kept so configs written before `time_format` existed still migrate: when
    /// `time_format` is absent this value decides, preserving the user's
    /// existing display rather than silently switching them to System.
    pub use_12h: bool,
    /// 24-hour, 12-hour, or follow the desktop. `None` means a config written
    /// before this field existed â migrate from `use_12h`.
    #[serde(default)]
    pub time_format: Option<crate::time_format::TimeFormat>,
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
            use_12h: false,
            time_format: Some(crate::time_format::TimeFormat::System),
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
    pub label: String,
    pub target: chrono::DateTime<chrono::Local>,
    pub yearly: bool,
    pub sound: String,
    /// Reminder offsets, stored by name so the set can grow without breaking
    /// existing configs â unknown names are dropped on load.
    pub reminders: Vec<String>,
    #[serde(default)]
    pub fired: Vec<String>,
    #[serde(default)]
    pub arrived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedWorkout {
    pub label: String,
    pub prep_secs: u32,
    pub work_secs: u32,
    pub rest_secs: u32,
    pub rounds: u32,
    pub sets: u32,
    pub set_rest_secs: u32,
    pub sound: String,
    /// Block structure. `None` marks a workout saved before blocks existed â
    /// those are lowered from the six scalars above on restore. The scalars are
    /// kept so a config written by this version still loads in an older build.
    #[serde(default)]
    pub blocks: Option<Vec<SavedBlock>>,
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
    pub hour: u8,
    pub minute: u8,
    pub label: String,
    pub is_enabled: bool,
    pub repeat_mode: SavedRepeatMode,
    pub sound: String,
    pub snooze_minutes: u8,
    pub ring_minutes: u8,
    /// Wall-clock time a pending snooze re-rings, `None` when not snoozed.
    ///
    /// `#[serde(default)]` keeps configs written before this field existed
    /// loadable. Note the struct `#[version]` must *not* be bumped for this:
    /// cosmic-config puts the version in the directory path, so a bump would
    /// start from an empty config and discard the user's existing data.
    #[serde(default)]
    pub snoozed_until: Option<chrono::DateTime<chrono::Local>>,
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
