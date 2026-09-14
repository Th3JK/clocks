// SPDX-License-Identifier: MIT

//! Alarm scheduling, as pure functions.
//!
//! Deliberately free of `AppModel`, widgets, audio and config writes so both
//! the GUI and the daemon can reason about the same schedule. The daemon is the
//! only thing that actually *fires* an alarm; the GUI uses `next_occurrence` to
//! tell the user when an alarm will next go off.

use crate::pages::alarm::{AlarmEntry, DayOfWeek, RepeatMode};
use crate::runtime::{RingingRecord, RuntimeState, SnoozeRecord};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone};

/// An alarm that has come due and should start ringing.
#[derive(Debug, Clone, PartialEq)]
pub struct DueAlarm {
    pub alarm_id: u32,
    pub label: String,
    pub sound: String,
    pub ring_secs: u64,
    pub snooze_minutes: u8,
}

impl DueAlarm {
    fn from_entry(a: &AlarmEntry) -> Self {
        Self {
            alarm_id: a.id,
            label: a.label.clone(),
            sound: a.sound.clone(),
            ring_secs: u64::from(a.ring_minutes) * 60,
            snooze_minutes: a.snooze_minutes,
        }
    }
}

/// Whether this alarm's repeat rule covers the given weekday.
fn fires_on(alarm: &AlarmEntry, weekday: chrono::Weekday) -> bool {
    match &alarm.repeat_mode {
        // A one-shot alarm fires on whatever day comes first; `consumed_once`
        // is what stops it firing again.
        RepeatMode::Once | RepeatMode::EveryDay => true,
        RepeatMode::Custom(days) => days.contains(&DayOfWeek::from_chrono(weekday)),
    }
}

/// Resolve a local date and the alarm's wall-clock time into an instant.
///
/// Returns `None` for a time that does not exist on that date -- the hour
/// skipped by a spring-forward DST transition. Ambiguous times, where the clock
/// repeats an hour, resolve to the earlier of the two rather than being dropped.
fn at_time(date: NaiveDate, alarm: &AlarmEntry) -> Option<DateTime<Local>> {
    let naive = date.and_hms_opt(u32::from(alarm.hour), u32::from(alarm.minute), 0)?;
    Local.from_local_datetime(&naive).earliest()
}

/// The next time this alarm is scheduled to fire, strictly after `after`.
///
/// `None` when nothing is scheduled -- a disabled alarm, a spent one-shot, or a
/// custom repeat with no days selected.
pub fn next_occurrence(
    alarm: &AlarmEntry,
    consumed_once: &[u32],
    after: DateTime<Local>,
) -> Option<DateTime<Local>> {
    if !alarm.is_enabled || consumed_once.contains(&alarm.id) {
        return None;
    }
    if matches!(&alarm.repeat_mode, RepeatMode::Custom(days) if days.is_empty()) {
        return None;
    }

    // A week is enough for every repeat mode, plus one day of slack so a
    // DST-skipped time can roll to the following day rather than vanishing.
    for days_ahead in 0..=8 {
        let date = (after + Duration::days(days_ahead)).date_naive();
        let Some(candidate) = at_time(date, alarm) else {
            continue;
        };
        if candidate > after && fires_on(alarm, candidate.weekday()) {
            return Some(candidate);
        }
    }
    None
}

/// Alarms whose scheduled time falls in `(since, now]`.
///
/// The window is what makes this resilient: if the daemon was asleep, suspended
/// or simply slow, anything that came due in the meantime still fires instead of
/// being silently skipped. `since` is capped at 24 hours so resuming from a long
/// suspend does not set off a day's worth of alarms at once.
pub fn due_alarms(
    alarms: &[AlarmEntry],
    state: &RuntimeState,
    since: DateTime<Local>,
    now: DateTime<Local>,
) -> Vec<DueAlarm> {
    let floor = now - Duration::hours(24);
    let since = since.max(floor);

    alarms
        .iter()
        .filter(|a| a.is_enabled && !state.consumed_once.contains(&a.id))
        // Already ringing, or waiting to re-ring after a snooze: leave alone.
        .filter(|a| !state.is_ringing(a.id) && state.snooze_for(a.id).is_none())
        .filter(|a| {
            // Today or yesterday is enough to cover a 24h window.
            (0..=1).any(|back| {
                let date = (now - Duration::days(back)).date_naive();
                at_time(date, a)
                    .is_some_and(|t| t > since && t <= now && fires_on(a, t.weekday()))
            })
        })
        .map(DueAlarm::from_entry)
        .collect()
}

/// Snoozes whose re-ring time has arrived.
pub fn due_snoozes(state: &RuntimeState, now: DateTime<Local>) -> Vec<SnoozeRecord> {
    state
        .snoozed
        .iter()
        .filter(|s| s.retrigger_at <= now)
        .cloned()
        .collect()
}

/// Ringing alarms that have been ringing longer than their window allows.
///
/// These auto-snooze rather than stopping outright, matching what the app has
/// always done when a ring is left unanswered.
pub fn expired_rings(state: &RuntimeState, now: DateTime<Local>) -> Vec<RingingRecord> {
    state
        .ringing
        .iter()
        .filter(|r| {
            now.signed_duration_since(r.started_at).num_seconds() >= r.ring_secs as i64
        })
        .cloned()
        .collect()
}

/// Move an alarm from ringing to snoozed.
pub fn snooze(state: &mut RuntimeState, alarm_id: u32, now: DateTime<Local>) {
    let Some(pos) = state.ringing.iter().position(|r| r.alarm_id == alarm_id) else {
        return;
    };
    let ringing = state.ringing.remove(pos);
    state.snoozed.retain(|s| s.alarm_id != alarm_id);
    state.snoozed.push(SnoozeRecord {
        alarm_id,
        label: ringing.label,
        sound: ringing.sound,
        ring_minutes: (ringing.ring_secs / 60) as u8,
        snooze_minutes: ringing.snooze_minutes,
        retrigger_at: now + Duration::minutes(i64::from(ringing.snooze_minutes)),
    });
}

/// Stop an alarm ringing and drop any pending snooze for it.
pub fn dismiss(state: &mut RuntimeState, alarm_id: u32) {
    state.ringing.retain(|r| r.alarm_id != alarm_id);
    state.snoozed.retain(|s| s.alarm_id != alarm_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alarm(hour: u8, minute: u8, repeat: RepeatMode) -> AlarmEntry {
        AlarmEntry {
            id: 1,
            hour,
            minute,
            label: "Test".into(),
            is_enabled: true,
            repeat_mode: repeat,
            sound: "Bell".into(),
            snooze_minutes: 5,
            ring_minutes: 1,
        }
    }

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Local> {
        Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(y, m, d)
                    .unwrap()
                    .and_hms_opt(h, min, 0)
                    .unwrap(),
            )
            .earliest()
            .unwrap()
    }

    #[test]
    fn next_occurrence_is_later_today() {
        let a = alarm(9, 0, RepeatMode::EveryDay);
        let now = at(2026, 9, 13, 7, 0);
        assert_eq!(next_occurrence(&a, &[], now), Some(at(2026, 9, 13, 9, 0)));
    }

    #[test]
    fn next_occurrence_rolls_to_tomorrow_once_passed() {
        let a = alarm(9, 0, RepeatMode::EveryDay);
        let now = at(2026, 9, 13, 9, 30);
        assert_eq!(next_occurrence(&a, &[], now), Some(at(2026, 9, 14, 9, 0)));
    }

    #[test]
    fn next_occurrence_skips_to_a_selected_weekday() {
        // 2026-09-13 is a Sunday; the next Wednesday is the 16th.
        let a = alarm(6, 30, RepeatMode::Custom(vec![DayOfWeek::Wednesday]));
        let now = at(2026, 9, 13, 12, 0);
        assert_eq!(next_occurrence(&a, &[], now), Some(at(2026, 9, 16, 6, 30)));
    }

    #[test]
    fn disabled_and_spent_alarms_have_no_next_occurrence() {
        let mut a = alarm(9, 0, RepeatMode::EveryDay);
        let now = at(2026, 9, 13, 7, 0);
        a.is_enabled = false;
        assert_eq!(next_occurrence(&a, &[], now), None);

        a.is_enabled = true;
        assert_eq!(next_occurrence(&a, &[1], now), None);
    }

    #[test]
    fn custom_repeat_with_no_days_never_fires() {
        let a = alarm(9, 0, RepeatMode::Custom(vec![]));
        let now = at(2026, 9, 13, 7, 0);
        assert_eq!(next_occurrence(&a, &[], now), None);
    }

    #[test]
    fn an_alarm_missed_while_asleep_still_fires() {
        // The whole point of the window: 09:00 passed while nothing was running.
        let a = alarm(9, 0, RepeatMode::EveryDay);
        let state = RuntimeState::default();
        let due = due_alarms(&[a], &state, at(2026, 9, 13, 8, 55), at(2026, 9, 13, 9, 5));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].alarm_id, 1);
        assert_eq!(due[0].ring_secs, 60);
    }

    #[test]
    fn an_alarm_does_not_fire_twice_for_the_same_occurrence() {
        let a = alarm(9, 0, RepeatMode::EveryDay);
        let state = RuntimeState::default();
        // The window has already moved past 09:00.
        let due = due_alarms(&[a], &state, at(2026, 9, 13, 9, 1), at(2026, 9, 13, 9, 5));
        assert!(due.is_empty());
    }

    #[test]
    fn a_ringing_or_snoozed_alarm_is_not_re_fired() {
        let a = alarm(9, 0, RepeatMode::EveryDay);
        let mut state = RuntimeState::default();
        state.snoozed.push(SnoozeRecord {
            alarm_id: 1,
            label: "Test".into(),
            sound: "Bell".into(),
            ring_minutes: 1,
            snooze_minutes: 5,
            retrigger_at: at(2026, 9, 13, 9, 5),
        });
        let due = due_alarms(&[a], &state, at(2026, 9, 13, 8, 55), at(2026, 9, 13, 9, 1));
        assert!(due.is_empty());
    }

    #[test]
    fn resuming_from_a_long_suspend_does_not_replay_the_week() {
        let a = alarm(9, 0, RepeatMode::EveryDay);
        let state = RuntimeState::default();
        // Five days asleep: only the most recent 09:00 counts, not five of them.
        let due = due_alarms(&[a], &state, at(2026, 9, 8, 0, 0), at(2026, 9, 13, 9, 5));
        assert_eq!(due.len(), 1);
    }

    #[test]
    fn snooze_moves_a_ring_and_sets_the_retrigger() {
        let mut state = RuntimeState::default();
        let now = at(2026, 9, 13, 9, 0);
        state.ringing.push(RingingRecord {
            alarm_id: 1,
            label: "Test".into(),
            sound: "Bell".into(),
            ring_secs: 60,
            snooze_minutes: 5,
            started_at: now,
        });

        snooze(&mut state, 1, now);

        assert!(state.ringing.is_empty());
        assert_eq!(state.snoozed.len(), 1);
        assert_eq!(state.snoozed[0].retrigger_at, at(2026, 9, 13, 9, 5));
    }

    #[test]
    fn dismiss_clears_both_ringing_and_snoozed() {
        let mut state = RuntimeState::default();
        let now = at(2026, 9, 13, 9, 0);
        state.ringing.push(RingingRecord {
            alarm_id: 1,
            label: "Test".into(),
            sound: "Bell".into(),
            ring_secs: 60,
            snooze_minutes: 5,
            started_at: now,
        });
        state.snoozed.push(SnoozeRecord {
            alarm_id: 1,
            label: "Test".into(),
            sound: "Bell".into(),
            ring_minutes: 1,
            snooze_minutes: 5,
            retrigger_at: now,
        });

        dismiss(&mut state, 1);

        assert!(state.ringing.is_empty());
        assert!(state.snoozed.is_empty());
    }

    #[test]
    fn a_ring_expires_once_its_window_elapses() {
        let mut state = RuntimeState::default();
        state.ringing.push(RingingRecord {
            alarm_id: 1,
            label: "Test".into(),
            sound: "Bell".into(),
            ring_secs: 60,
            snooze_minutes: 5,
            started_at: at(2026, 9, 13, 9, 0),
        });

        assert!(expired_rings(&state, at(2026, 9, 13, 9, 0)).is_empty());
        assert_eq!(expired_rings(&state, at(2026, 9, 13, 9, 1)).len(), 1);
    }
}
