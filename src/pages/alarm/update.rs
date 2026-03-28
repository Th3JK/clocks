// SPDX-License-Identifier: MIT
//
// Alarm update logic: message handling, trigger checks, snooze, and ringing.

use super::model::*;
use super::Message;
use crate::fl;
use std::time::Instant;

/// Convert 24h hour (0-23) to 12h display hour (1-12) + `is_pm`
pub(super) fn hour24_to_12(hour24: u8) -> (u8, bool) {
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
        (12, false) => 0,    // 12 AM = 0
        (12, true) => 12,    // 12 PM = 12
        (h, false) => h,     // 1-11 AM = 1-11
        (h, true) => h + 12, // 1-11 PM = 13-23
    }
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
                let (hour, is_pm) = (8, false);
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
                    edit.snooze_minutes = m.clamp(1, 30);
                }
            }
            Message::EditRingMinutes(m) => {
                if let Some(edit) = &mut self.editing {
                    edit.ring_minutes = m.clamp(1, 30);
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
            Message::ToggleEditMode => {
                self.edit_mode = !self.edit_mode;
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::StartDrag(index) => {
                self.pre_drag_order = self.alarms.iter().map(|a| a.id).collect();
                self.dragging_index = Some(index);
            }
            Message::Reorder(from, to) => {
                if from < self.alarms.len() && to < self.alarms.len() && from != to {
                    let alarm = self.alarms.remove(from);
                    self.alarms.insert(to, alarm);
                    self.dragging_index = Some(to);
                }
            }
            Message::FinishDrag => {
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::CancelDrag => {
                if !self.pre_drag_order.is_empty() {
                    let id_order = &self.pre_drag_order;
                    let mut restored = Vec::with_capacity(id_order.len());
                    for &id in id_order {
                        if let Some(pos) = self.alarms.iter().position(|a| a.id == id) {
                            restored.push(self.alarms.remove(pos));
                        }
                    }
                    restored.append(&mut self.alarms);
                    self.alarms = restored;
                }
                self.dragging_index = None;
                self.pre_drag_order.clear();
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
                        retrigger_at: Instant::now()
                            + std::time::Duration::from_secs(snooze_secs),
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
}
