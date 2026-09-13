// SPDX-License-Identifier: MIT
//
// Config ↔ runtime state conversion: building a `Config` from page states
// and restoring page states from a saved `Config`.

use crate::config::{
    Config, PomodoroDayStat, PomodoroDefaults, SavedAlarm, SavedChessConfig, SavedClock, SavedLap,
    SavedPomodoro, SavedRepeatMode, SavedStopwatchRecord, SavedTimer, SavedWorkout,
};
use crate::pages::{alarm, chess, pomodoro, stopwatch, timer, workout, world_clocks};
use std::time::Duration;

// --- Persistence: build Config from runtime state ---

#[allow(clippy::too_many_arguments)]
pub(super) fn build_config_from_state(
    wc: &world_clocks::WorldClocksState,
    al: &alarm::AlarmState,
    ti: &timer::TimerState,
    po: &pomodoro::PomodoroState,
    sw: &stopwatch::StopwatchState,
    ch: &chess::ChessState,
    wo: &workout::WorkoutState,
    use_12h: bool,
    confirm_delete_alarm: bool,
    confirm_delete_timer: bool,
    confirm_delete_world_clock: bool,
    confirm_delete_pomodoro: bool,
    confirm_clear_stopwatch: bool,
    auto_sort_alarms: bool,
    auto_sort_world_clocks: bool,
    auto_clear_stopwatch_history: bool,
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
                snoozed_until: al
                    .snoozed
                    .iter()
                    .find(|s| s.alarm_id == a.id)
                    .map(|s| s.retrigger_at),
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

    let pomodoro_stats = po
        .daily_stats
        .iter()
        .map(|d| PomodoroDayStat {
            date: d.date.format("%Y-%m-%d").to_string(),
            focus_secs: d.focus_secs,
            sessions: d.sessions,
        })
        .collect();

    let stopwatch_history = sw
        .history
        .iter()
        .map(|r| SavedStopwatchRecord {
            label: r.label.clone(),
            total_elapsed_ms: r.total_elapsed.as_millis() as u64,
            laps: r
                .laps
                .iter()
                .map(|l| SavedLap {
                    lap_time_ms: l.lap_time.as_millis() as u64,
                    delta_ms: l.delta,
                })
                .collect(),
        })
        .collect();

    let chess = SavedChessConfig {
        base_minutes: ch.base_minutes,
        increment_secs: ch.increment_secs,
    };

    let workouts = wo
        .workouts
        .iter()
        .map(|w| SavedWorkout {
            label: w.label.clone(),
            // The legacy scalars are no longer the source of truth; they are
            // written as a benign fallback so an older build can still open the
            // config without seeing a zero-length workout.
            prep_secs: 0,
            work_secs: 30,
            rest_secs: 10,
            rounds: 8,
            sets: 1,
            set_rest_secs: 60,
            sound: w.sound.clone(),
            blocks: Some(w.blocks.iter().map(save_block).collect()),
        })
        .collect();

    let countdown_events = co
        .events
        .iter()
        .map(|e| SavedCountdownEvent {
            label: e.label.clone(),
            target: e.target,
            yearly: e.yearly,
            sound: e.sound.clone(),
            reminders: e.reminders.iter().map(|r| r.key().to_string()).collect(),
            fired: e.fired.iter().map(|r| r.key().to_string()).collect(),
            arrived: e.arrived,
        })
        .collect();

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
        auto_sort_alarms,
        auto_sort_world_clocks,
        auto_clear_stopwatch_history,
        stopwatch_history,
        pomodoro_stats,
        chess,
        workouts,
    }
}

pub(super) fn restore_chess(config: &Config) -> chess::ChessState {
    chess::ChessState::new(config.chess.base_minutes, config.chess.increment_secs)
}

pub(super) fn restore_workouts(config: &Config) -> workout::WorkoutState {
    if config.workouts.is_empty() {
        return workout::WorkoutState::default();
    }

    let mut state = workout::WorkoutState {
        workouts: Vec::new(),
        ..Default::default()
    };
    for (i, w) in config.workouts.iter().enumerate() {
        // Workouts saved before blocks existed carry `None` here; lower their
        // six scalars into the equivalent block layout so they behave exactly
        // as they did before.
        let blocks = match &w.blocks {
            Some(blocks) => blocks.iter().map(load_block).collect(),
            None => workout::simple_blocks(
                w.prep_secs,
                w.work_secs,
                w.rest_secs,
                w.rounds,
                w.sets,
                w.set_rest_secs,
            ),
        };
        state.workouts.push(workout::WorkoutEntry::new(
            (i + 1) as u32,
            w.label.clone(),
            blocks,
            w.sound.clone(),
        ));
    }
    // Derive from the highest id in use rather than the count: positional ids
    // collide with a live id after deletions.
    state.next_id = state.workouts.iter().map(|w| w.id).max().unwrap_or(0) + 1;
    state
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
        edit_mode: false,
        dragging_index: None,
        pre_drag_order: Vec::new(),
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

    // Ids are positional, so derive the next id from the highest in use rather
    // than the count. With deletions in play `len() + 1` can collide with a
    // live id, which now matters because snoozes reference alarms by id.
    let next_id = alarms.iter().map(|a| a.id).max().unwrap_or(0) + 1;

    // Rebuild pending snoozes from the saved re-ring times. Everything else the
    // snooze needs is already on the alarm itself, so only the time is stored.
    // A snooze whose time passed while the app was closed is dropped rather than
    // fired retroactively.
    let now = chrono::Local::now();
    let snoozed = config
        .alarms
        .iter()
        .zip(alarms.iter())
        .filter_map(|(s, entry)| {
            let retrigger_at = s.snoozed_until?;
            (retrigger_at > now).then(|| alarm::SnoozedAlarm {
                alarm_id: entry.id,
                label: entry.label.clone(),
                sound: entry.sound.clone(),
                ring_minutes: entry.ring_minutes,
                snooze_minutes: entry.snooze_minutes,
                retrigger_at,
            })
        })
        .collect();

    alarm::AlarmState {
        alarms,
        next_id,
        editing: None,
        last_triggered_minute: None,
        ringing: Vec::new(),
        snoozed,
        edit_mode: false,
        dragging_index: None,
        pre_drag_order: Vec::new(),
        last_saved_id: None,
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
        edit_mode: false,
        dragging_index: None,
        pre_drag_order: Vec::new(),
        focused_id: None,
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

    state.daily_stats = config
        .pomodoro_stats
        .iter()
        .filter_map(|d| {
            chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
                .ok()
                .map(|date| pomodoro::DayStat {
                    date,
                    focus_secs: d.focus_secs,
                    sessions: d.sessions,
                })
        })
        .collect();
    state.daily_stats.sort_by_key(|d| d.date);

    state
}

pub(super) fn restore_stopwatch_history(config: &Config) -> stopwatch::StopwatchState {
    let history: Vec<stopwatch::StopwatchRecord> = config
        .stopwatch_history
        .iter()
        .enumerate()
        .map(|(i, r)| stopwatch::StopwatchRecord {
            id: (i + 1) as u32,
            label: r.label.clone(),
            total_elapsed: Duration::from_millis(r.total_elapsed_ms),
            laps: r
                .laps
                .iter()
                .enumerate()
                .map(|(j, l)| stopwatch::LapEntry {
                    id: (j + 1) as u32,
                    lap_time: Duration::from_millis(l.lap_time_ms),
                    delta: l.delta_ms,
                    is_fastest: false,
                    is_slowest: false,
                })
                .collect(),
        })
        .collect();

    // Recompute fastest/slowest flags for each record's laps
    let history: Vec<stopwatch::StopwatchRecord> = history
        .into_iter()
        .map(|mut r| {
            if r.laps.len() >= 2 {
                let min = r.laps.iter().map(|l| l.lap_time).min().unwrap();
                let max = r.laps.iter().map(|l| l.lap_time).max().unwrap();
                for lap in &mut r.laps {
                    lap.is_fastest = lap.lap_time == min;
                    lap.is_slowest = lap.lap_time == max;
                }
            }
            r
        })
        .collect();

    let next_history_id = history.len() as u32 + 1;

    stopwatch::StopwatchState {
        history,
        next_history_id,
        ..Default::default()
    }
}
