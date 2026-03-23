// SPDX-License-Identifier: MIT
//
// Alarm data types: entries, trigger info, ringing/snooze state, editing state.

use crate::fl;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum RepeatMode {
    Once,
    EveryDay,
    Custom(Vec<DayOfWeek>),
}

impl RepeatMode {
    pub fn display_name(&self) -> String {
        match self {
            RepeatMode::Once => fl!("once"),
            RepeatMode::EveryDay => fl!("every-day"),
            RepeatMode::Custom(days) => {
                let day_strs: Vec<String> = days.iter().map(|d| d.display_name()).collect();
                day_strs.join(" ")
            }
        }
    }
}

impl std::fmt::Display for RepeatMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DayOfWeek {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl DayOfWeek {
    /// English short name for serialization (config persistence)
    pub fn short_name(&self) -> &'static str {
        match self {
            DayOfWeek::Monday => "Mon",
            DayOfWeek::Tuesday => "Tue",
            DayOfWeek::Wednesday => "Wed",
            DayOfWeek::Thursday => "Thu",
            DayOfWeek::Friday => "Fri",
            DayOfWeek::Saturday => "Sat",
            DayOfWeek::Sunday => "Sun",
        }
    }

    /// Localized short name for display
    pub fn display_name(&self) -> String {
        match self {
            DayOfWeek::Monday => fl!("day-mon"),
            DayOfWeek::Tuesday => fl!("day-tue"),
            DayOfWeek::Wednesday => fl!("day-wed"),
            DayOfWeek::Thursday => fl!("day-thu"),
            DayOfWeek::Friday => fl!("day-fri"),
            DayOfWeek::Saturday => fl!("day-sat"),
            DayOfWeek::Sunday => fl!("day-sun"),
        }
    }

    pub fn all() -> &'static [DayOfWeek] {
        &[
            DayOfWeek::Monday,
            DayOfWeek::Tuesday,
            DayOfWeek::Wednesday,
            DayOfWeek::Thursday,
            DayOfWeek::Friday,
            DayOfWeek::Saturday,
            DayOfWeek::Sunday,
        ]
    }

    pub fn from_chrono(weekday: chrono::Weekday) -> Self {
        match weekday {
            chrono::Weekday::Mon => DayOfWeek::Monday,
            chrono::Weekday::Tue => DayOfWeek::Tuesday,
            chrono::Weekday::Wed => DayOfWeek::Wednesday,
            chrono::Weekday::Thu => DayOfWeek::Thursday,
            chrono::Weekday::Fri => DayOfWeek::Friday,
            chrono::Weekday::Sat => DayOfWeek::Saturday,
            chrono::Weekday::Sun => DayOfWeek::Sunday,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlarmEntry {
    pub id: u32,
    pub hour: u8,
    pub minute: u8,
    pub label: String,
    pub is_enabled: bool,
    pub repeat_mode: RepeatMode,
    pub sound: String,
    pub snooze_minutes: u8,
    pub ring_minutes: u8,
}

/// Info returned when an alarm triggers
#[derive(Debug, Clone)]
pub struct AlarmTriggerInfo {
    pub alarm_id: u32,
    pub label: String,
    pub sound: String,
    pub ring_secs: u64,
    pub snooze_minutes: u8,
}

/// A currently ringing alarm
pub struct RingingAlarm {
    pub alarm_id: u32,
    pub label: String,
    pub sound: String,
    pub ring_secs: u64,
    pub snooze_minutes: u8,
    pub started_at: Instant,
}

/// An alarm waiting to re-ring after snooze
pub struct SnoozedAlarm {
    pub alarm_id: u32,
    pub label: String,
    pub sound: String,
    pub ring_minutes: u8,
    pub snooze_minutes: u8,
    pub retrigger_at: Instant,
}

pub struct AlarmState {
    pub alarms: Vec<AlarmEntry>,
    pub next_id: u32,
    pub editing: Option<AlarmEdit>,
    pub last_triggered_minute: Option<(u8, u8)>,
    pub ringing: Vec<RingingAlarm>,
    pub snoozed: Vec<SnoozedAlarm>,
}

#[derive(Debug, Clone)]
pub struct AlarmEdit {
    pub id: Option<u32>,
    pub hour: u8,
    pub minute: u8,
    pub is_pm: bool,
    pub label: String,
    pub repeat_mode: RepeatMode,
    pub sound: String,
    pub snooze_minutes: u8,
    pub ring_minutes: u8,
}

impl Default for AlarmState {
    fn default() -> Self {
        Self {
            alarms: Vec::new(),
            next_id: 1,
            editing: None,
            last_triggered_minute: None,
            ringing: Vec::new(),
            snoozed: Vec::new(),
        }
    }
}
