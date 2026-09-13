// SPDX-License-Identifier: MIT
//
// Config ↔ runtime state conversion: building a `Config` from page states
// and restoring page states from a saved `Config`.

use crate::config::{
    Config, PomodoroDayStat, PomodoroDefaults, SavedAlarm, SavedBlock, SavedChessConfig,
    SavedClock, SavedCountdownEvent, SavedLap, SavedPomodoro, SavedRepeatMode, SavedStep,
    SavedStepKind, SavedStopwatchRecord, SavedTimer, SavedWorkout,
};
use crate::pages::{alarm, chess, countdown, pomodoro, stopwatch, timer, workout, world_clocks};
use std::time::Duration;

// --- Persistence: build Config from runtime state ---

#[allow(clippy::too_many_arguments)]
pub fn build_config_from_state(
    wc: &world_clocks::WorldClocksState,
    al: &alarm::AlarmState,
    ti: &timer::TimerState,
    po: &pomodoro::PomodoroState,
    sw: &stopwatch::StopwatchState,
    ch: &chess::ChessState,
    wo: &workout::WorkoutState,
    co: &countdown::CountdownState,
    nav_order: &[crate::pages::Page],
    nav_hidden: &[crate::pages::Page],
    time_format: crate::time_format::TimeFormat,
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
                id: a.id,
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
            id: t.id,
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
            sound: w.sound.clone(),
            blocks: w.blocks.iter().map(save_block).collect(),
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
        // The legacy flag mirrors the preference only when it is concrete; a
        // System preference leaves it at its last explicit value so an older
        // build still gets something sensible.
        time_format,
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
        countdown_events,
        nav_order: nav_order.iter().map(|p| p.key().to_string()).collect(),
        nav_hidden: nav_hidden.iter().map(|p| p.key().to_string()).collect(),
    }
}

/// Sidebar order and hidden set from the config.
///
/// An empty stored order means the sidebar was never customised, so fall back
/// to the built-in order. Any page missing from a stored order is appended:
/// that is how a page added in a later release shows up instead of silently
/// vanishing for anyone with a saved layout.
pub fn restore_nav(config: &Config) -> (Vec<crate::pages::Page>, Vec<crate::pages::Page>) {
    use crate::pages::Page;
    let mut order: Vec<Page> = config
        .nav_order
        .iter()
        .filter_map(|k| Page::from_key(k))
        .collect();
    for page in Page::ALL {
        if !order.contains(&page) {
            order.push(page);
        }
    }
    let hidden = config
        .nav_hidden
        .iter()
        .filter_map(|k| Page::from_key(k))
        .collect();
    (order, hidden)
}

pub fn restore_chess(config: &Config) -> chess::ChessState {
    chess::ChessState::new(config.chess.base_minutes, config.chess.increment_secs)
}

pub fn restore_countdowns(config: &Config) -> countdown::CountdownState {
    let mut state = countdown::CountdownState::default();
    for (i, e) in config.countdown_events.iter().enumerate() {
        let mut event =
            countdown::CountdownEvent::new((i + 1) as u32, e.label.clone(), e.target);
        event.yearly = e.yearly;
        event.sound = e.sound.clone();
        // Unknown reminder names are dropped rather than failing the load, so
        // the preset list can change without invalidating saved events.
        event.reminders = e
            .reminders
            .iter()
            .filter_map(|k| countdown::Reminder::from_key(k))
            .collect();
        event.fired = e
            .fired
            .iter()
            .filter_map(|k| countdown::Reminder::from_key(k))
            .collect();
        event.arrived = e.arrived;
        state.events.push(event);
    }
    // From the highest id in use: positional ids collide after deletions.
    state.next_id = state.events.iter().map(|e| e.id).max().unwrap_or(0) + 1;
    state
}

fn save_step_kind(kind: workout::StepKind) -> SavedStepKind {
    match kind {
        workout::StepKind::Prep => SavedStepKind::Prep,
        workout::StepKind::Effort => SavedStepKind::Effort,
        workout::StepKind::Recovery => SavedStepKind::Recovery,
    }
}

fn load_step_kind(kind: SavedStepKind) -> workout::StepKind {
    match kind {
        SavedStepKind::Prep => workout::StepKind::Prep,
        SavedStepKind::Effort => workout::StepKind::Effort,
        SavedStepKind::Recovery => workout::StepKind::Recovery,
    }
}

fn save_block(block: &workout::Block) -> SavedBlock {
    SavedBlock {
        repeat: block.repeat,
        skip_last_recovery: block.skip_last_recovery,
        steps: block
            .steps
            .iter()
            .map(|s| SavedStep {
                label: s.label.clone(),
                secs: s.secs,
                kind: save_step_kind(s.kind),
            })
            .collect(),
    }
}

fn load_block(block: &SavedBlock) -> workout::Block {
    workout::Block::new(
        block.repeat,
        block
            .steps
            .iter()
            .map(|s| workout::Step::new(s.label.clone(), s.secs, load_step_kind(s.kind)))
            .collect(),
        block.skip_last_recovery,
    )
}

pub fn restore_workouts(config: &Config) -> workout::WorkoutState {
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
        let blocks = w.blocks.iter().map(load_block).collect();
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

pub fn restore_world_clocks(config: &Config) -> world_clocks::WorldClocksState {
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

pub fn restore_alarms(config: &Config) -> alarm::AlarmState {
    let alarms: Vec<alarm::AlarmEntry> = config
        .alarms
        .iter()
        .map(|a| {
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
            alarm::AlarmEntry {
                id: a.id,
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

    // From the highest id in use, never the count: with deletions in play
    // `len() + 1` can collide with a live id.
    let next_id = alarms.iter().map(|a| a.id).max().unwrap_or(0) + 1;

    // Snoozes live in the daemon's runtime state, not here -- the GUI receives
    // them through `Message::UpdateRuntime`.
    let snoozed = Vec::new();

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

pub fn restore_timers(config: &Config) -> timer::TimerState {
    let timers: Vec<timer::TimerEntry> = config
        .timers
        .iter()
        .map(|t| {
            let dur = Duration::from_secs(t.duration_secs);
            timer::TimerEntry {
                id: t.id,
                label: t.label.clone(),
                initial_duration: dur,
                remaining: dur,
                is_running: false,
                start_instant: None,
                started_remaining: dur,
                repeat_enabled: t.repeat_enabled,
                repeat_count: t.repeat_count,
                completed_count: 0,
                sound: t.sound.clone(),
            }
        })
        .collect();

    // From the highest id in use, not the count: with deletions in play
    // `len() + 1` collides with a live id.
    let next_id = timers.iter().map(|t| t.id).max().unwrap_or(0) + 1;

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

pub fn restore_pomodoros(config: &Config) -> pomodoro::PomodoroState {
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
            timer.sound = p.sound.clone();
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

pub fn restore_stopwatch_history(config: &Config) -> stopwatch::StopwatchState {
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
