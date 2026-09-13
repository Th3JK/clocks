// SPDX-License-Identifier: MIT
//
// Countdown update logic: editing, and the once-a-second reminder/arrival check.

use super::Message;
use super::model::*;
use crate::fl;
use chrono::{Duration, Local, NaiveDate, Timelike};

impl CountdownState {
    /// Update and return (notification body, sound) pairs for reminders and
    /// arrivals.
    pub fn update(&mut self, message: Message) -> Vec<(String, String)> {
        let mut notifications = Vec::new();

        match message {
            Message::Tick => {
                let now = Local::now();
                // The global tick runs at 100 ms. Date arithmetic over every
                // event that often is waste, and firing from it would deliver
                // the same reminder up to ten times within one second.
                if self.last_check.map(|t| t.timestamp()) == Some(now.timestamp()) {
                    return notifications;
                }
                self.last_check = Some(now);

                for event in &mut self.events {
                    // Reminders, soonest threshold last so a long-passed event
                    // doesn't emit a burst out of order.
                    let mut due: Vec<Reminder> = event
                        .reminders
                        .iter()
                        .copied()
                        .filter(|r| {
                            !event.fired.contains(r)
                                && event.seconds_until(now) <= r.secs_before()
                                && !event.has_passed(now)
                        })
                        .collect();
                    due.sort_by_key(|r| std::cmp::Reverse(r.secs_before()));
                    for reminder in due {
                        event.fired.push(reminder);
                        notifications.push((
                            fl!(
                                "countdown-reminder-body",
                                label = event.label.clone(),
                                when = reminder.display_name()
                            ),
                            event.sound.clone(),
                        ));
                    }

                    // Arrival.
                    if event.has_passed(now) && !event.arrived {
                        event.arrived = true;
                        notifications.push((
                            fl!("countdown-arrived", label = event.label.clone()),
                            event.sound.clone(),
                        ));
                    }

                    // A yearly event re-arms itself for next year once it has
                    // arrived, rather than counting up forever.
                    event.roll_forward(now);
                }
            }
            Message::Delete(id) => {
                self.events.retain(|e| e.id != id);
                if self.editing_id == Some(id) {
                    self.editing_id = None;
                }
            }
            Message::OpenSettings => {
                // Opening the drawer always means "new". Without this, opening
                // it after an edit would reopen that event's form and saving
                // would silently overwrite it.
                let now = Local::now();
                let tomorrow = to_jiff((now + Duration::days(1)).date_naive());
                self.editing_id = None;
                self.edit_label.clear();
                self.edit_calendar = cosmic::widget::calendar::CalendarModel::new(tomorrow, tomorrow);
                self.show_calendar = false;
                self.edit_hour = 9;
                self.edit_minute = 0;
                self.edit_yearly = false;
                self.edit_sound = "Bell".to_string();
                self.edit_reminders = vec![Reminder::OneHour];
            }
            Message::StartEditEvent(id) => {
                if let Some(e) = self.events.iter().find(|e| e.id == id) {
                    self.editing_id = Some(id);
                    self.edit_label = e.label.clone();
                    let jd = to_jiff(e.target.date_naive());
                    self.edit_calendar = cosmic::widget::calendar::CalendarModel::new(jd, jd);
                    self.show_calendar = false;
                    self.edit_hour = e.target.hour();
                    self.edit_minute = e.target.minute();
                    self.edit_yearly = e.yearly;
                    self.edit_sound = e.sound.clone();
                    self.edit_reminders = e.reminders.clone();
                }
            }
            Message::AddEvent => {
                if let Some(target) = self.edit_target() {
                    let label = if self.edit_label.trim().is_empty() {
                        fl!("countdown-default-label", id = self.next_id.to_string())
                    } else {
                        self.edit_label.clone()
                    };
                    let mut event = CountdownEvent::new(self.next_id, label, target);
                    event.yearly = self.edit_yearly;
                    event.sound = self.edit_sound.clone();
                    event.reminders = self.edit_reminders.clone();
                    self.events.push(event);
                    self.next_id += 1;
                    self.edit_label.clear();
                }
            }
            Message::SaveEditEvent => {
                if let Some(id) = self.editing_id.take()
                    && let Some(target) = self.edit_target()
                    && let Some(e) = self.events.iter_mut().find(|e| e.id == id)
                {
                    if !self.edit_label.trim().is_empty() {
                        e.label = self.edit_label.clone();
                    }
                    e.target = target;
                    e.yearly = self.edit_yearly;
                    e.sound = self.edit_sound.clone();
                    e.reminders = self.edit_reminders.clone();
                    // Re-arm: the new target may be further out than the old one,
                    // so previously-delivered reminders should fire again.
                    e.fired.clear();
                    e.arrived = false;
                }
                self.edit_label.clear();
            }
            Message::CancelEdit => {
                self.editing_id = None;
                self.edit_label.clear();
            }
            Message::EditLabel(label) => self.edit_label = label,
            Message::EditDate(y, m, d) => {
                // Validated here rather than in the view closure, which cannot
                // capture state and must stay 'static.
                if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
                    self.edit_calendar.set_selected_visible(to_jiff(date));
                    // Picking a date is the whole point of the popup; close it.
                    self.show_calendar = false;
                }
            }
            Message::ToggleCalendar => self.show_calendar = !self.show_calendar,
            Message::ShowPrevMonth => self.edit_calendar.show_prev_month(),
            Message::ShowNextMonth => self.edit_calendar.show_next_month(),
            Message::EditHour(h) => self.edit_hour = h % 24,
            Message::EditMinute(m) => self.edit_minute = m % 60,
            Message::ToggleYearly => self.edit_yearly = !self.edit_yearly,
            Message::ToggleReminder(r) => {
                if let Some(pos) = self.edit_reminders.iter().position(|x| *x == r) {
                    self.edit_reminders.remove(pos);
                } else {
                    self.edit_reminders.push(r);
                }
            }
            Message::EditSound(sound) => self.edit_sound = sound,
            Message::BrowseCustomSound => {
                // Handled in app.rs
            }
            Message::ToggleEditMode => self.edit_mode = !self.edit_mode,
        }

        notifications
    }
}
