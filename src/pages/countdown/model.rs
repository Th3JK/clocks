// SPDX-License-Identifier: MIT
//
// Countdown-to-event data types.
//
// Events are wall-clock targets, so everything here is chrono. The calendar
// widget libcosmic provides speaks `jiff::civil::Date`; that conversion is kept
// at the view boundary (see `to_jiff` / `from_jiff` in `view.rs`) so the model
// never has to care.

use crate::fl;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone};

/// How far ahead of an event a reminder fires. A fixed set rather than a free
/// duration — these cover the realistic cases without an extra form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reminder {
    FiveMin,
    FifteenMin,
    OneHour,
    OneDay,
    OneWeek,
}

impl Reminder {
    pub const ALL: [Reminder; 5] = [
        Reminder::FiveMin,
        Reminder::FifteenMin,
        Reminder::OneHour,
        Reminder::OneDay,
        Reminder::OneWeek,
    ];

    pub fn secs_before(self) -> i64 {
        match self {
            Reminder::FiveMin => 5 * 60,
            Reminder::FifteenMin => 15 * 60,
            Reminder::OneHour => 60 * 60,
            Reminder::OneDay => 24 * 60 * 60,
            Reminder::OneWeek => 7 * 24 * 60 * 60,
        }
    }

    /// Stable name used in the config, so the preset list can grow or be
    /// reordered without invalidating saved events.
    pub fn key(self) -> &'static str {
        match self {
            Reminder::FiveMin => "5min",
            Reminder::FifteenMin => "15min",
            Reminder::OneHour => "1hour",
            Reminder::OneDay => "1day",
            Reminder::OneWeek => "1week",
        }
    }

    pub fn from_key(key: &str) -> Option<Reminder> {
        Reminder::ALL.into_iter().find(|r| r.key() == key)
    }

    pub fn display_name(self) -> String {
        match self {
            Reminder::FiveMin => fl!("countdown-reminder-5min"),
            Reminder::FifteenMin => fl!("countdown-reminder-15min"),
            Reminder::OneHour => fl!("countdown-reminder-1hour"),
            Reminder::OneDay => fl!("countdown-reminder-1day"),
            Reminder::OneWeek => fl!("countdown-reminder-1week"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CountdownEvent {
    pub id: u32,
    pub label: String,
    pub target: DateTime<Local>,
    /// Roll the target forward a year each time it passes.
    pub yearly: bool,
    pub sound: String,
    pub reminders: Vec<Reminder>,
    /// Reminders already delivered for the current target, so each fires once.
    /// Cleared when a yearly event rolls over.
    pub fired: Vec<Reminder>,
    /// Whether the arrival notification has been sent for the current target.
    pub arrived: bool,
}

impl CountdownEvent {
    pub fn new(id: u32, label: String, target: DateTime<Local>) -> Self {
        Self {
            id,
            label,
            target,
            yearly: false,
            sound: "Bell".to_string(),
            reminders: Vec::new(),
            fired: Vec::new(),
            arrived: false,
        }
    }

    /// Seconds until the target. Negative once it has passed.
    pub fn seconds_until(&self, now: DateTime<Local>) -> i64 {
        (self.target - now).num_seconds()
    }

    pub fn has_passed(&self, now: DateTime<Local>) -> bool {
        self.seconds_until(now) <= 0
    }

    /// Move a yearly event to its next occurrence and re-arm its notifications.
    /// Returns true when it actually rolled.
    pub fn roll_forward(&mut self, now: DateTime<Local>) -> bool {
        if !self.yearly || !self.has_passed(now) {
            return false;
        }
        // Step a year at a time so an event that has been passed for several
        // years still lands on the next future occurrence.
        while self.has_passed(now) {
            match next_year(self.target) {
                Some(next) => self.target = next,
                // Only reachable at the edge of chrono's range; stop rather than
                // spin forever.
                None => return false,
            }
        }
        self.fired.clear();
        self.arrived = false;
        true
    }
}

/// The same clock time one year later. 29 February falls back to the 28th,
/// which has no valid date in a non-leap year.
fn next_year(dt: DateTime<Local>) -> Option<DateTime<Local>> {
    let year = dt.year() + 1;
    let date = NaiveDate::from_ymd_opt(year, dt.month(), dt.day())
        .or_else(|| NaiveDate::from_ymd_opt(year, dt.month(), dt.day() - 1))?;
    let naive = date.and_time(dt.time());
    Local.from_local_datetime(&naive).single().or_else(|| {
        // Ambiguous or skipped local time (a DST boundary). Nudge an hour.
        Local
            .from_local_datetime(&(naive + Duration::hours(1)))
            .single()
    })
}

pub struct CountdownState {
    pub events: Vec<CountdownEvent>,
    pub next_id: u32,
    // Editing state (context drawer)
    pub editing_id: Option<u32>,
    pub edit_label: String,
    /// Date selection and visible month for the picker.
    ///
    /// This is libcosmic's own model, in jiff types, because `widget::calendar`
    /// borrows it for the lifetime of the returned element — it cannot be built
    /// from a local. It is the only jiff in the app; `edit_date()` converts back
    /// to chrono for everything else.
    pub edit_calendar: cosmic::widget::calendar::CalendarModel,
    /// Whether the calendar popup is open. The calendar is tall, so it is shown
    /// on demand behind the date button rather than permanently in the drawer.
    pub show_calendar: bool,
    pub edit_hour: u32,
    pub edit_minute: u32,
    pub edit_yearly: bool,
    pub edit_sound: String,
    pub edit_reminders: Vec<Reminder>,
    // Edit mode (delete)
    pub edit_mode: bool,
    /// Wall-clock second the tick last did real work, so the 100 ms global tick
    /// doesn't redo date arithmetic ten times a second.
    pub last_check: Option<DateTime<Local>>,
}

/// chrono -> jiff, for libcosmic's calendar model.
pub fn to_jiff(date: NaiveDate) -> jiff::civil::Date {
    jiff::civil::Date::new(date.year() as i16, date.month() as i8, date.day() as i8)
        .unwrap_or_else(|_| jiff::civil::Date::new(2000, 1, 1).expect("valid date"))
}

/// jiff -> chrono, for reading the calendar's selection back out.
pub fn from_jiff(date: jiff::civil::Date) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(
        i32::from(date.year()),
        u32::from(date.month() as u8),
        u32::from(date.day() as u8),
    )
}

impl Default for CountdownState {
    fn default() -> Self {
        let now = Local::now();
        let tomorrow = (now + Duration::days(1)).date_naive();
        let jd = to_jiff(tomorrow);
        Self {
            events: Vec::new(),
            next_id: 1,
            editing_id: None,
            edit_label: String::new(),
            edit_calendar: cosmic::widget::calendar::CalendarModel::new(jd, jd),
            show_calendar: false,
            edit_hour: 9,
            edit_minute: 0,
            edit_yearly: false,
            edit_sound: "Bell".to_string(),
            edit_reminders: vec![Reminder::OneHour],
            edit_mode: false,
            last_check: None,
        }
    }
}

impl CountdownState {
    /// Whether the tick has anything left to do: an event still to arrive, a
    /// reminder still to fire, or a yearly event waiting to roll over.
    pub fn has_pending(&self) -> bool {
        let now = Local::now();
        self.events.iter().any(|e| {
            e.yearly || !e.arrived || e.fired.len() < e.reminders.len() || !e.has_passed(now)
        })
    }

    /// The selected date, back in chrono.
    pub fn edit_date(&self) -> Option<NaiveDate> {
        from_jiff(self.edit_calendar.selected)
    }

    /// Build a `DateTime<Local>` from the edit fields, falling back to the next
    /// representable local time if the combination is skipped by a DST jump.
    pub fn edit_target(&self) -> Option<DateTime<Local>> {
        let naive = self
            .edit_date()?
            .and_hms_opt(self.edit_hour, self.edit_minute, 0)?;
        Local
            .from_local_datetime(&naive)
            .single()
            .or_else(|| Local.from_local_datetime(&(naive + Duration::hours(1))).single())
    }
}
