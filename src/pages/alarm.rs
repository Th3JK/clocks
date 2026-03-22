// SPDX-License-Identifier: MIT

use crate::components::sound_selector_view;
use crate::fl;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;
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

#[derive(Debug, Clone)]
pub enum Message {
    ToggleAlarm(u32),
    DeleteAlarm(u32),
    StartNewAlarm,
    StartEditAlarm(u32),
    CancelEdit,
    SaveAlarm,
    IncrementHour,
    DecrementHour,
    IncrementMinute,
    DecrementMinute,
    EditLabel(String),
    EditRepeatOnce,
    EditRepeatEveryDay,
    ToggleDay(DayOfWeek),
    EditSound(String),
    EditSnoozeMinutes(u8),
    EditRingMinutes(u8),
    BrowseCustomSound,
    ToggleAmPm(bool),
    SnoozeAlarm(u32),
    DismissAlarm(u32),
}

impl AlarmState {
    pub fn update(&mut self, message: Message, use_12h: bool) {
        match message {
            Message::ToggleAlarm(id) => {
                if let Some(alarm) = self.alarms.iter_mut().find(|a| a.id == id) {
                    alarm.is_enabled = !alarm.is_enabled;
                }
            }
            Message::DeleteAlarm(id) => {
                self.alarms.retain(|a| a.id != id);
            }
            Message::StartNewAlarm => {
                let (hour, is_pm) = if use_12h {
                    (8, false) // 8 AM
                } else {
                    (8, false)
                };
                self.editing = Some(AlarmEdit {
                    id: None,
                    hour,
                    minute: 0,
                    is_pm,
                    label: String::new(),
                    repeat_mode: RepeatMode::Once,
                    sound: "Bell".to_string(),
                    snooze_minutes: 5,
                    ring_minutes: 1,
                });
            }
            Message::StartEditAlarm(id) => {
                if let Some(alarm) = self.alarms.iter().find(|a| a.id == id) {
                    let (hour, is_pm) = if use_12h {
                        hour24_to_12(alarm.hour)
                    } else {
                        (alarm.hour, false)
                    };
                    self.editing = Some(AlarmEdit {
                        id: Some(alarm.id),
                        hour,
                        minute: alarm.minute,
                        is_pm,
                        label: alarm.label.clone(),
                        repeat_mode: alarm.repeat_mode.clone(),
                        sound: alarm.sound.clone(),
                        snooze_minutes: alarm.snooze_minutes,
                        ring_minutes: alarm.ring_minutes,
                    });
                }
            }
            Message::CancelEdit => {
                self.editing = None;
            }
            Message::SaveAlarm => {
                if let Some(edit) = self.editing.take() {
                    let saved_hour = if use_12h {
                        hour12_to_24(edit.hour, edit.is_pm)
                    } else {
                        edit.hour
                    };
                    if let Some(id) = edit.id {
                        if let Some(alarm) = self.alarms.iter_mut().find(|a| a.id == id) {
                            alarm.hour = saved_hour;
                            alarm.minute = edit.minute;
                            alarm.label = edit.label;
                            alarm.repeat_mode = edit.repeat_mode;
                            alarm.sound = edit.sound;
                            alarm.snooze_minutes = edit.snooze_minutes;
                            alarm.ring_minutes = edit.ring_minutes;
                        }
                    } else {
                        self.alarms.push(AlarmEntry {
                            id: self.next_id,
                            hour: saved_hour,
                            minute: edit.minute,
                            label: if edit.label.is_empty() {
                                fl!("alarm-default-label")
                            } else {
                                edit.label
                            },
                            is_enabled: true,
                            repeat_mode: edit.repeat_mode,
                            sound: edit.sound,
                            snooze_minutes: edit.snooze_minutes,
                            ring_minutes: edit.ring_minutes,
                        });
                        self.next_id += 1;
                    }
                }
            }
            Message::IncrementHour => {
                if let Some(edit) = &mut self.editing {
                    if use_12h {
                        edit.hour = if edit.hour == 12 { 1 } else { edit.hour + 1 };
                    } else {
                        edit.hour = (edit.hour + 1) % 24;
                    }
                }
            }
            Message::DecrementHour => {
                if let Some(edit) = &mut self.editing {
                    if use_12h {
                        edit.hour = if edit.hour == 1 { 12 } else { edit.hour - 1 };
                    } else {
                        edit.hour = if edit.hour == 0 { 23 } else { edit.hour - 1 };
                    }
                }
            }
            Message::IncrementMinute => {
                if let Some(edit) = &mut self.editing {
                    edit.minute = (edit.minute + 1) % 60;
                }
            }
            Message::DecrementMinute => {
                if let Some(edit) = &mut self.editing {
                    edit.minute = if edit.minute == 0 {
                        59
                    } else {
                        edit.minute - 1
                    };
                }
            }
            Message::EditLabel(label) => {
                if let Some(edit) = &mut self.editing {
                    edit.label = label;
                }
            }
            Message::EditRepeatOnce => {
                if let Some(edit) = &mut self.editing {
                    edit.repeat_mode = RepeatMode::Once;
                }
            }
            Message::EditRepeatEveryDay => {
                if let Some(edit) = &mut self.editing {
                    edit.repeat_mode = RepeatMode::EveryDay;
                }
            }
            Message::EditSound(sound) => {
                if let Some(edit) = &mut self.editing {
                    edit.sound = sound;
                }
            }
            Message::EditSnoozeMinutes(m) => {
                if let Some(edit) = &mut self.editing {
                    edit.snooze_minutes = m.max(1).min(30);
                }
            }
            Message::EditRingMinutes(m) => {
                if let Some(edit) = &mut self.editing {
                    edit.ring_minutes = m.max(1).min(30);
                }
            }
            Message::ToggleAmPm(is_pm) => {
                if let Some(edit) = &mut self.editing {
                    edit.is_pm = is_pm;
                }
            }
            Message::BrowseCustomSound => {
                // Handled in app.rs
            }
            Message::ToggleDay(day) => {
                if let Some(edit) = &mut self.editing {
                    match &mut edit.repeat_mode {
                        RepeatMode::Custom(days) => {
                            if let Some(pos) = days.iter().position(|d| *d == day) {
                                days.remove(pos);
                                if days.is_empty() {
                                    edit.repeat_mode = RepeatMode::Once;
                                }
                            } else {
                                days.push(day);
                            }
                        }
                        _ => {
                            edit.repeat_mode = RepeatMode::Custom(vec![day]);
                        }
                    }
                }
            }
            Message::SnoozeAlarm(alarm_id) => {
                if let Some(pos) = self.ringing.iter().position(|r| r.alarm_id == alarm_id) {
                    let ringing = self.ringing.remove(pos);
                    let snooze_secs = ringing.snooze_minutes as u64 * 60;
                    self.snoozed.push(SnoozedAlarm {
                        alarm_id: ringing.alarm_id,
                        label: ringing.label,
                        sound: ringing.sound,
                        ring_minutes: (ringing.ring_secs / 60).max(1) as u8,
                        snooze_minutes: ringing.snooze_minutes,
                        retrigger_at: Instant::now() + std::time::Duration::from_secs(snooze_secs),
                    });
                }
            }
            Message::DismissAlarm(alarm_id) => {
                self.ringing.retain(|r| r.alarm_id != alarm_id);
            }
        }
    }

    /// Check if any alarms should trigger right now. Returns trigger info.
    pub fn check_triggers(
        &mut self,
        hour: u8,
        minute: u8,
        weekday: chrono::Weekday,
    ) -> Vec<AlarmTriggerInfo> {
        let current = (hour, minute);
        let mut triggered = Vec::new();

        // Only check scheduled alarms once per minute
        if self.last_triggered_minute != Some(current) {
            self.last_triggered_minute = Some(current);
            let dow = DayOfWeek::from_chrono(weekday);

            for alarm in &mut self.alarms {
                if !alarm.is_enabled {
                    continue;
                }
                if alarm.hour == hour && alarm.minute == minute {
                    let should_trigger = match &alarm.repeat_mode {
                        RepeatMode::Once => true,
                        RepeatMode::EveryDay => true,
                        RepeatMode::Custom(days) => days.contains(&dow),
                    };
                    if should_trigger {
                        triggered.push(AlarmTriggerInfo {
                            alarm_id: alarm.id,
                            label: alarm.label.clone(),
                            sound: alarm.sound.clone(),
                            ring_secs: alarm.ring_minutes as u64 * 60,
                            snooze_minutes: alarm.snooze_minutes,
                        });
                        if alarm.repeat_mode == RepeatMode::Once {
                            alarm.is_enabled = false;
                        }
                    }
                }
            }
        }

        triggered
    }

    /// Check snoozed alarms and return any that should re-trigger now
    pub fn check_snoozed(&mut self) -> Vec<AlarmTriggerInfo> {
        let now = Instant::now();
        let mut retriggers = Vec::new();
        let mut remaining = Vec::new();

        for snoozed in self.snoozed.drain(..) {
            if now >= snoozed.retrigger_at {
                retriggers.push(AlarmTriggerInfo {
                    alarm_id: snoozed.alarm_id,
                    label: snoozed.label,
                    sound: snoozed.sound,
                    ring_secs: snoozed.ring_minutes as u64 * 60,
                    snooze_minutes: snoozed.snooze_minutes,
                });
            } else {
                remaining.push(snoozed);
            }
        }

        self.snoozed = remaining;
        retriggers
    }

    /// Check for ringing alarms whose ring duration has expired.
    /// Returns alarm IDs that should be auto-snoozed.
    pub fn check_ring_expired(&self) -> Vec<u32> {
        let now = Instant::now();
        self.ringing
            .iter()
            .filter(|r| now.duration_since(r.started_at).as_secs() >= r.ring_secs)
            .map(|r| r.alarm_id)
            .collect()
    }

    /// Start ringing an alarm
    pub fn start_ringing(&mut self, info: &AlarmTriggerInfo) {
        // Don't double-ring the same alarm
        if self.ringing.iter().any(|r| r.alarm_id == info.alarm_id) {
            return;
        }
        self.ringing.push(RingingAlarm {
            alarm_id: info.alarm_id,
            label: info.label.clone(),
            sound: info.sound.clone(),
            ring_secs: info.ring_secs,
            snooze_minutes: info.snooze_minutes,
            started_at: Instant::now(),
        });
    }

    /// Main view: page header + alarm list
    pub fn view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(self.alarms.len() + 3)
            .spacing(spacing);

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("alarms-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .on_press(Message::StartNewAlarm),
            );
        col = col.push(header);

        // Ringing alarms are now shown via the floating dialog (Application::dialog())

        if self.alarms.is_empty() {
            col = col.push(
                widget::container(widget::text::body(fl!("no-alarms")))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .padding(24),
            );
        }

        for alarm in &self.alarms {
            let time_str = if use_12h {
                let (h12, is_pm) = hour24_to_12(alarm.hour);
                let period = if is_pm { fl!("pm") } else { fl!("am") };
                format!("{:02}:{:02} {}", h12, alarm.minute, period)
            } else {
                format!("{:02}:{:02}", alarm.hour, alarm.minute)
            };

            let id = alarm.id;
            let row = widget::row::with_capacity(5)
                .spacing(spacing)
                .align_y(Alignment::Center)
                .push(
                    widget::column::with_capacity(3)
                        .push(widget::text::body(&alarm.label))
                        .push(widget::text::title3(time_str))
                        .push(widget::text::caption(format!(
                            "{}",
                            alarm.repeat_mode
                        )))
                        .width(Length::Fill),
                )
                .push(
                    widget::button::icon(widget::icon::from_name("edit-symbolic"))
                        .on_press(Message::StartEditAlarm(id)),
                )
                .push(
                    widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                        .on_press(Message::DeleteAlarm(id)),
                )
                .push(
                    widget::toggler(alarm.is_enabled)
                        .on_toggle(move |_| Message::ToggleAlarm(id)),
                );

            col = col.push(row);
        }

        col.into()
    }

    /// Sidebar view: alarm editing form
    pub fn sidebar_view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(10).spacing(spacing);

        if let Some(edit) = &self.editing {
            // Label
            col = col.push(widget::text::body(fl!("label")));
            col = col
                .push(widget::text_input(fl!("alarm-label-placeholder"), &edit.label).on_input(Message::EditLabel));

            // Time spinners with wrap-around
            let hour_str = format!("{:02}", edit.hour);
            let minute_str = format!("{:02}", edit.minute);

            col = col.push(widget::text::body(fl!("time")));
            let time_row = widget::row::with_capacity(8)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                        .on_press(Message::DecrementHour),
                )
                .push(widget::text::title3(hour_str))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                        .on_press(Message::IncrementHour),
                )
                .push(widget::text::title3(":"))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                        .on_press(Message::DecrementMinute),
                )
                .push(widget::text::title3(minute_str))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                        .on_press(Message::IncrementMinute),
                );
            col = col.push(time_row);

            // AM/PM selector (only in 12h mode)
            if use_12h {
                let am_btn = if edit.is_pm {
                    widget::button::standard(fl!("am")).on_press(Message::ToggleAmPm(false))
                } else {
                    widget::button::suggested(fl!("am")).on_press(Message::ToggleAmPm(false))
                };
                let pm_btn = if edit.is_pm {
                    widget::button::suggested(fl!("pm")).on_press(Message::ToggleAmPm(true))
                } else {
                    widget::button::standard(fl!("pm")).on_press(Message::ToggleAmPm(true))
                };
                let ampm_row = widget::row::with_capacity(2)
                    .spacing(8)
                    .push(am_btn)
                    .push(pm_btn);
                col = col.push(ampm_row);
            }

            // Repeat mode with highlighted selection
            col = col.push(widget::text::body(fl!("repeat")));

            let is_once = matches!(edit.repeat_mode, RepeatMode::Once);
            let is_everyday = matches!(edit.repeat_mode, RepeatMode::EveryDay);

            let once_btn = if is_once {
                widget::button::suggested(fl!("once")).on_press(Message::EditRepeatOnce)
            } else {
                widget::button::standard(fl!("once")).on_press(Message::EditRepeatOnce)
            };
            let everyday_btn = if is_everyday {
                widget::button::suggested(fl!("every-day")).on_press(Message::EditRepeatEveryDay)
            } else {
                widget::button::standard(fl!("every-day")).on_press(Message::EditRepeatEveryDay)
            };

            let repeat_row = widget::row::with_capacity(2)
                .spacing(8)
                .push(once_btn)
                .push(everyday_btn);
            col = col.push(repeat_row);

            // Day toggles
            col = col.push(widget::text::caption(fl!("select-specific-days")));
            let selected_days = match &edit.repeat_mode {
                RepeatMode::Custom(days) => days.clone(),
                _ => Vec::new(),
            };
            let mut days_row = widget::row::with_capacity(7).spacing(4);
            for day in DayOfWeek::all() {
                let is_selected = selected_days.contains(day);
                if is_selected {
                    days_row = days_row.push(
                        widget::button::suggested(day.display_name())
                            .on_press(Message::ToggleDay(*day)),
                    );
                } else {
                    days_row = days_row.push(
                        widget::button::standard(day.display_name())
                            .on_press(Message::ToggleDay(*day)),
                    );
                }
            }
            col = col.push(days_row);

            // Snooze duration
            col = col.push(widget::divider::horizontal::default());
            col = col.push(widget::text::body(fl!("snooze-duration")));
            let snz = edit.snooze_minutes;
            let snooze_row = widget::row::with_capacity(3)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                        .on_press(Message::EditSnoozeMinutes(snz.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = snz.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                        .on_press(Message::EditSnoozeMinutes(snz + 1)),
                );
            col = col.push(snooze_row);

            // Ring duration
            col = col.push(widget::text::body(fl!("ring-duration")));
            let ring = edit.ring_minutes;
            let ring_row = widget::row::with_capacity(3)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                        .on_press(Message::EditRingMinutes(ring.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = ring.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                        .on_press(Message::EditRingMinutes(ring + 1)),
                );
            col = col.push(ring_row);

            // Sound selection
            col = col.push(widget::divider::horizontal::default());
            col = col.push(sound_selector_view(
                fl!("sound"),
                &edit.sound,
                Message::EditSound,
                Message::BrowseCustomSound,
            ));

            // Save/Cancel
            col = col.push(widget::divider::horizontal::default());
            let actions = widget::row::with_capacity(2)
                .spacing(8)
                .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEdit))
                .push(widget::button::suggested(fl!("save")).on_press(Message::SaveAlarm));
            col = col.push(actions);
        }

        col.into()
    }
}

/// Convert 24h hour (0-23) to 12h display hour (1-12) + `is_pm`
fn hour24_to_12(hour24: u8) -> (u8, bool) {
    let is_pm = hour24 >= 12;
    let h12 = match hour24 {
        0 => 12,
        1..=12 => hour24,
        _ => hour24 - 12,
    };
    (h12, is_pm)
}

/// Convert 12h display hour (1-12) + `is_pm` to 24h hour (0-23)
fn hour12_to_24(hour12: u8, is_pm: bool) -> u8 {
    match (hour12, is_pm) {
        (12, false) => 0,   // 12 AM = 0
        (12, true) => 12,   // 12 PM = 12
        (h, false) => h,    // 1-11 AM = 1-11
        (h, true) => h + 12, // 1-11 PM = 13-23
    }
}
