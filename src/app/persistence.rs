// SPDX-License-Identifier: MIT
//
// Config ↔ runtime state conversion: building a `Config` from page states
// and restoring page states from a saved `Config`.

use crate::config::{
    Config, PomodoroDefaults, SavedAlarm, SavedClock, SavedPomodoro, SavedRepeatMode, SavedTimer,
};
use crate::pages::{alarm, pomodoro, timer, world_clocks};
use std::time::Duration;

// --- Persistence: build Config from runtime state ---

#[allow(clippy::too_many_arguments)]
pub(super) fn build_config_from_state(
    wc: &world_clocks::WorldClocksState,
    al: &alarm::AlarmState,
    ti: &timer::TimerState,
    po: &pomodoro::PomodoroState,
    use_12h: bool,
    confirm_delete_alarm: bool,
    confirm_delete_timer: bool,
    confirm_delete_world_clock: bool,
    confirm_delete_pomodoro: bool,
    confirm_clear_stopwatch: bool,
) -> Config {
    let world_clocks = wc
        .clocks
        .iter()
        .map(|c| SavedClock {
            timezone: c.timezone,
            city_name: c.city_name.clone(),
            is_local: c.is_local,
        })
        .collect();

    let alarms = al
        .alarms
        .iter()
        .map(|a| {
            let repeat_mode = match &a.repeat_mode {
                alarm::RepeatMode::Once => SavedRepeatMode::Once,
                alarm::RepeatMode::EveryDay => SavedRepeatMode::EveryDay,
                alarm::RepeatMode::Custom(days) => SavedRepeatMode::Custom(
                    days.iter().map(|d| d.short_name().to_string()).collect(),
                ),
            };
            SavedAlarm {
                hour: a.hour,
                minute: a.minute,
                label: a.label.clone(),
                is_enabled: a.is_enabled,
                repeat_mode,
                sound: a.sound.clone(),
                snooze_minutes: a.snooze_minutes,
                ring_minutes: a.ring_minutes,
            }
        })
        .collect();

    let timers = ti
        .timers
        .iter()
        .map(|t| SavedTimer {
            label: t.label.clone(),
            duration_secs: t.initial_duration.as_secs(),
            repeat_enabled: t.repeat_enabled,
            repeat_count: t.repeat_count,
            sound: t.sound.clone(),
        })
        .collect();

    let pomodoros = po
        .timers
        .iter()
        .map(|p| SavedPomodoro {
            label: p.label.clone(),
            work_minutes: p.work_minutes,
            short_break_minutes: p.short_break_minutes,
            long_break_minutes: p.long_break_minutes,
            sound: p.sound.clone(),
        })
        .collect();

    let pomodoro_defaults = PomodoroDefaults {
        work_minutes: po.default_work_minutes,
        short_break_minutes: po.default_short_break_minutes,
        long_break_minutes: po.default_long_break_minutes,
    };

    Config {
        world_clocks,
        alarms,
        timers,
        pomodoros,
        pomodoro_defaults,
        use_12h,
        confirm_delete_alarm,
        confirm_delete_timer,
        confirm_delete_world_clock,
        confirm_delete_pomodoro,
        confirm_clear_stopwatch,
    }
}

// --- Persistence: restore runtime state from Config ---

pub(super) fn restore_world_clocks(config: &Config) -> world_clocks::WorldClocksState {
    if config.world_clocks.is_empty() {
        return world_clocks::WorldClocksState::default();
    }

    let clocks: Vec<world_clocks::ClockEntry> = config
        .world_clocks
        .iter()
        .enumerate()
        .map(|(i, c)| world_clocks::ClockEntry {
            id: i as u32,
            timezone: c.timezone,
            city_name: c.city_name.clone(),
            is_local: c.is_local,
        })
        .collect();

    let local_tz = clocks
        .iter()
        .find(|c| c.is_local)
        .map(|c| c.timezone)
        .unwrap_or_else(|| {
            iana_time_zone::get_timezone()
                .ok()
                .and_then(|tz_str| tz_str.parse().ok())
                .unwrap_or(chrono_tz::UTC)
        });

    let next_id = clocks.len() as u32;

    world_clocks::WorldClocksState {
        local_timezone: local_tz,
        clocks,
        next_id,
        search_text: String::new(),
        filtered_timezones: Vec::new(),
        selected_clock_id: None,
    }
}

pub(super) fn restore_alarms(config: &Config) -> alarm::AlarmState {
    let alarms: Vec<alarm::AlarmEntry> = config
        .alarms
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let repeat_mode = match &a.repeat_mode {
                SavedRepeatMode::Once => alarm::RepeatMode::Once,
                SavedRepeatMode::EveryDay => alarm::RepeatMode::EveryDay,
                SavedRepeatMode::Custom(days) => {
                    let parsed: Vec<alarm::DayOfWeek> = days
                        .iter()
                        .filter_map(|d| match d.as_str() {
                            "Mon" => Some(alarm::DayOfWeek::Monday),
                            "Tue" => Some(alarm::DayOfWeek::Tuesday),
                            "Wed" => Some(alarm::DayOfWeek::Wednesday),
                            "Thu" => Some(alarm::DayOfWeek::Thursday),
                            "Fri" => Some(alarm::DayOfWeek::Friday),
                            "Sat" => Some(alarm::DayOfWeek::Saturday),
                            "Sun" => Some(alarm::DayOfWeek::Sunday),
                            _ => None,
                        })
                        .collect();
                    if parsed.is_empty() {
                        alarm::RepeatMode::Once
                    } else {
                        alarm::RepeatMode::Custom(parsed)
                    }
                }
            };
            // Migrate "Default" sound to "Bell"
            let sound = if a.sound == "Default" {
                "Bell".to_string()
            } else {
                a.sound.clone()
            };
            alarm::AlarmEntry {
                id: (i + 1) as u32,
                hour: a.hour,
                minute: a.minute,
                label: a.label.clone(),
                is_enabled: a.is_enabled,
                repeat_mode,
                sound,
                snooze_minutes: a.snooze_minutes,
                ring_minutes: a.ring_minutes,
            }
        })
        .collect();

    let next_id = alarms.len() as u32 + 1;

    alarm::AlarmState {
        alarms,
        next_id,
        editing: None,
        last_triggered_minute: None,
        ringing: Vec::new(),
        snoozed: Vec::new(),
    }
}

pub(super) fn restore_timers(config: &Config) -> timer::TimerState {
    let timers: Vec<timer::TimerEntry> = config
        .timers
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let dur = Duration::from_secs(t.duration_secs);
            // Migrate "Default" sound to "Bell"
            let sound = if t.sound == "Default" {
                "Bell".to_string()
            } else {
                t.sound.clone()
            };
            timer::TimerEntry {
                id: (i + 1) as u32,
                label: t.label.clone(),
                initial_duration: dur,
                remaining: dur,
                is_running: false,
                start_instant: None,
                started_remaining: dur,
                repeat_enabled: t.repeat_enabled,
                repeat_count: t.repeat_count,
                completed_count: 0,
                sound,
            }
        })
        .collect();

    let next_id = timers.len() as u32 + 1;

    timer::TimerState {
        timers,
        next_id,
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

pub(super) fn restore_pomodoros(config: &Config) -> pomodoro::PomodoroState {
    let mut state = pomodoro::PomodoroState {
        default_work_minutes: config.pomodoro_defaults.work_minutes,
        default_short_break_minutes: config.pomodoro_defaults.short_break_minutes,
        default_long_break_minutes: config.pomodoro_defaults.long_break_minutes,
        ..Default::default()
    };

    if !config.pomodoros.is_empty() {
        state.timers.clear();
        for (i, p) in config.pomodoros.iter().enumerate() {
            let mut timer = pomodoro::PomodoroTimer::from_config(
                i as u32,
                p.label.clone(),
                p.work_minutes,
                p.short_break_minutes,
                p.long_break_minutes,
            );
            // Migrate "Default" sound to "Bell"
            timer.sound = if p.sound == "Default" {
                "Bell".to_string()
            } else {
                p.sound.clone()
            };
            state.timers.push(timer);
        }
        state.next_id = config.pomodoros.len() as u32;
    }

    state
}
