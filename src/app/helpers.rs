// SPDX-License-Identifier: MIT
//
// Private helper methods on `AppModel`: tick handling, page shortcuts,
// state persistence, audio control, and title updates.

use super::persistence::build_config_from_state;
use super::{AppModel, Message};
use crate::audio;
use crate::fl;
use crate::pages::{Page, alarm, pomodoro, stopwatch, timer};
use cosmic::cosmic_config::CosmicConfigEntry;
use chrono::{Datelike, Local, NaiveTime, Offset, TimeZone, Timelike, Utc};
use cosmic::prelude::*;
use cosmic::widget::{self, toaster};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

impl AppModel {
    /// Central tick handler: drives stopwatch, timers, pomodoro, and alarm logic
    pub(super) fn handle_tick(&mut self) {
        // Stopwatch tick
        if self.stopwatch.is_running {
            self.stopwatch.update(stopwatch::Message::Tick);
        }

        // Timer tick + completion notifications
        if self.timer.has_running_timers() {
            let completed = self.timer.update(timer::Message::Tick);
            for (label, sound) in completed {
                audio::send_notification(&fl!("notification-timer-complete"), &label);
                audio::play_sound(&sound);
            }
        }

        // Pomodoro tick + session transition notifications
        if self.pomodoro.is_running() {
            let notifications = self.pomodoro.update(pomodoro::Message::Tick);
            for (msg, sound) in notifications {
                audio::send_notification(&fl!("notification-pomodoro"), &msg);
                audio::play_sound(&sound);
            }
        }

        // Alarm: check for expired ringing (auto-snooze)
        let expired = self.alarm.check_ring_expired();
        for alarm_id in expired {
            self.stop_alarm_audio(alarm_id);
            self.alarm
                .update(alarm::Message::SnoozeAlarm(alarm_id), self.use_12h);
        }

        // Alarm: check snoozed alarms that should re-trigger
        let snoozed_triggers = self.alarm.check_snoozed();
        for info in &snoozed_triggers {
            audio::send_notification(&fl!("notification-alarm-snoozed"), &info.label);
            self.start_alarm_audio(info);
            self.alarm.start_ringing(info);
        }
        if !snoozed_triggers.is_empty() {
            self.save_state();
        }

        // Alarm: check scheduled alarms
        let now = Local::now();
        let triggered =
            self.alarm
                .check_triggers(now.hour() as u8, now.minute() as u8, now.weekday());
        if !triggered.is_empty() {
            for info in &triggered {
                audio::send_notification(&fl!("notification-alarm"), &info.label);
                self.start_alarm_audio(info);
                self.alarm.start_ringing(info);
            }
            self.save_state();
        }
    }

    /// Sort alarms by time (hour, minute).
    pub(super) fn sort_alarms(&mut self) {
        self.alarm
            .alarms
            .sort_by(|a, b| (a.hour, a.minute).cmp(&(b.hour, b.minute)));
    }

    /// Sort world clocks by UTC offset so that:
    /// - clocks behind (west) come first (most negative offset),
    /// - the local timezone sits in the middle,
    /// - clocks ahead (east) come last (most positive offset).
    pub(super) fn sort_world_clocks(&mut self) {
        let now_utc = Utc::now();
        let local_tz = self.world_clocks.local_timezone;
        let local_offset = local_tz
            .offset_from_utc_datetime(&now_utc.naive_utc())
            .fix()
            .local_minus_utc();

        self.world_clocks.clocks.sort_by(|a, b| {
            let off_a = a
                .timezone
                .offset_from_utc_datetime(&now_utc.naive_utc())
                .fix()
                .local_minus_utc()
                - local_offset;
            let off_b = b
                .timezone
                .offset_from_utc_datetime(&now_utc.naive_utc())
                .fix()
                .local_minus_utc()
                - local_offset;
            off_a.cmp(&off_b)
        });
    }

    pub(super) fn save_state(&mut self) {
        if self.auto_sort_alarms {
            self.sort_alarms();
        }
        if self.auto_sort_world_clocks {
            self.sort_world_clocks();
        }
        let Some(ctx) = &self.config_context else {
            return;
        };
        let config = build_config_from_state(
            &self.world_clocks,
            &self.alarm,
            &self.timer,
            &self.pomodoro,
            self.use_12h,
            self.confirm_delete_alarm,
            self.confirm_delete_timer,
            self.confirm_delete_world_clock,
            self.confirm_delete_pomodoro,
            self.confirm_clear_stopwatch,
            self.auto_sort_alarms,
            self.auto_sort_world_clocks,
        );
        if let Err(e) = config.write_entry(ctx) {
            eprintln!("Failed to save config: {:?}", e);
        }
    }

    pub(super) fn start_alarm_audio(&mut self, info: &alarm::AlarmTriggerInfo) {
        let stop = Arc::new(AtomicBool::new(false));
        self.alarm_audio_stops.insert(info.alarm_id, stop.clone());
        let sound = info.sound.clone();
        let ring_secs = info.ring_secs;
        std::thread::spawn(move || {
            if let Err(e) = audio::play_alarm_sound_loop(&sound, ring_secs, stop) {
                eprintln!("Alarm audio error: {}", e);
            }
        });
    }

    pub(super) fn stop_alarm_audio(&mut self, alarm_id: u32) {
        if let Some(stop) = self.alarm_audio_stops.remove(&alarm_id) {
            stop.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn active_timer(&self) -> Option<&timer::TimerEntry> {
        self.active_timer_id
            .and_then(|id| self.timer.timers.iter().find(|t| t.id == id))
            .or_else(|| self.timer.timers.first())
    }

    pub(super) fn active_pomodoro(&self) -> Option<&pomodoro::PomodoroTimer> {
        self.active_pomodoro_id
            .and_then(|id| self.pomodoro.timers.iter().find(|p| p.id == id))
            .or_else(|| self.pomodoro.timers.first())
    }

    pub(super) fn handle_page_shortcut_space(&mut self) -> Task<cosmic::Action<Message>> {
        match self.nav.active_data::<Page>() {
            Some(Page::Stopwatch) => {
                if self.stopwatch.is_running {
                    self.stopwatch.update(stopwatch::Message::Stop);
                } else {
                    self.stopwatch.update(stopwatch::Message::Start);
                }
                self.save_state();
            }
            Some(Page::Timer) => {
                if let Some(t) = self.active_timer() {
                    let id = t.id;
                    let msg = if t.is_running {
                        timer::Message::PauseTimer(id)
                    } else if t.remaining < t.initial_duration {
                        timer::Message::ResumeTimer(id)
                    } else {
                        timer::Message::StartTimer(id)
                    };
                    self.active_timer_id = Some(id);
                    self.timer.update(msg);
                    self.save_state();
                }
            }
            Some(Page::Pomodoro) => {
                if let Some(p) = self.active_pomodoro() {
                    let id = p.id;
                    let msg = if p.is_running {
                        pomodoro::Message::Pause(id)
                    } else if p.remaining < p.started_remaining {
                        pomodoro::Message::Resume(id)
                    } else {
                        pomodoro::Message::Start(id)
                    };
                    self.active_pomodoro_id = Some(id);
                    self.pomodoro.update(msg);
                    self.save_state();
                }
            }
            _ => {}
        }
        Task::none()
    }

    pub(super) fn handle_page_shortcut_enter(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(Page::Stopwatch) = self.nav.active_data::<Page>()
            && self.stopwatch.is_running
        {
            self.stopwatch.update(stopwatch::Message::Lap);
            self.save_state();
        }
        Task::none()
    }

    pub(super) fn handle_page_shortcut_delete(&mut self) -> Task<cosmic::Action<Message>> {
        match self.nav.active_data::<Page>() {
            Some(Page::Stopwatch) => {
                if !self.stopwatch.is_running
                    && self.stopwatch.elapsed > std::time::Duration::ZERO
                {
                    self.stopwatch.update(stopwatch::Message::Reset);
                    self.save_state();
                }
            }
            Some(Page::Timer) => {
                if let Some(t) = self.active_timer() {
                    let id = t.id;
                    if !t.is_running && t.remaining < t.initial_duration {
                        self.timer.update(timer::Message::ResetTimer(id));
                        self.save_state();
                    }
                }
            }
            Some(Page::Pomodoro) => {
                if let Some(p) = self.active_pomodoro() {
                    let id = p.id;
                    if !p.is_running {
                        self.pomodoro.update(pomodoro::Message::Reset(id));
                        self.save_state();
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }

    pub(super) fn handle_page_shortcut_ctrl_n(&mut self) -> Task<cosmic::Action<Message>> {
        match self.nav.active_data::<Page>() {
            Some(Page::WorldClocks) => {
                self.context_page = crate::pages::ContextPage::WorldClocksAdd;
                self.core.window.show_context = true;
                self.save_state();
                return widget::text_input::focus(widget::Id::new(
                    "world-clocks-search-input",
                ));
            }
            Some(Page::Alarm) => {
                self.alarm.update(alarm::Message::StartNewAlarm, self.use_12h);
                self.context_page = crate::pages::ContextPage::AlarmEdit;
                self.core.window.show_context = true;
                self.save_state();
                return widget::text_input::focus(widget::Id::new("alarm-label-input"));
            }
            Some(Page::Timer) => {
                self.timer.update(timer::Message::StartNew);
                self.context_page = crate::pages::ContextPage::TimerAdd;
                self.core.window.show_context = true;
                self.save_state();
                return widget::text_input::focus(widget::Id::new("timer-label-input"));
            }
            Some(Page::Pomodoro) => {
                self.pomodoro.update(pomodoro::Message::AddTimer);
                self.save_state();
            }
            _ => {}
        }
        Task::none()
    }

    pub(super) fn handle_page_shortcut_skip(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(Page::Pomodoro) = self.nav.active_data::<Page>()
            && let Some(p) = self.active_pomodoro()
        {
            let id = p.id;
            if p.is_running
                && matches!(
                    p.session_type,
                    pomodoro::SessionType::ShortBreak | pomodoro::SessionType::LongBreak
                )
            {
                self.active_pomodoro_id = Some(id);
                self.pomodoro.update(pomodoro::Message::Skip(id));
                self.save_state();
            }
        }
        Task::none()
    }

    /// Push a toast showing how long until the given alarm fires.
    pub(super) fn push_alarm_toast(&mut self, alarm: &alarm::AlarmEntry) -> Task<cosmic::Action<Message>> {
        let now = Local::now();
        let alarm_time = NaiveTime::from_hms_opt(alarm.hour as u32, alarm.minute as u32, 0)
            .unwrap_or_default();
        let now_time = now.time();

        // Compute minutes until next occurrence
        let total_minutes = if alarm_time > now_time {
            // Later today
            let diff = alarm_time - now_time;
            diff.num_minutes()
        } else {
            // Tomorrow (or next scheduled day)
            let diff = alarm_time - now_time;
            diff.num_minutes() + 24 * 60
        };

        // Account for day-of-week scheduling
        let total_minutes = match &alarm.repeat_mode {
            alarm::RepeatMode::Custom(days) if !days.is_empty() => {
                let today = alarm::DayOfWeek::from_chrono(now.weekday());
                let today_works = days.contains(&today) && alarm_time > now_time;

                if today_works {
                    let diff = alarm_time - now_time;
                    diff.num_minutes()
                } else {
                    // Find next matching day
                    let weekdays: Vec<chrono::Weekday> = days
                        .iter()
                        .map(|d| match d {
                            alarm::DayOfWeek::Monday => chrono::Weekday::Mon,
                            alarm::DayOfWeek::Tuesday => chrono::Weekday::Tue,
                            alarm::DayOfWeek::Wednesday => chrono::Weekday::Wed,
                            alarm::DayOfWeek::Thursday => chrono::Weekday::Thu,
                            alarm::DayOfWeek::Friday => chrono::Weekday::Fri,
                            alarm::DayOfWeek::Saturday => chrono::Weekday::Sat,
                            alarm::DayOfWeek::Sunday => chrono::Weekday::Sun,
                        })
                        .collect();

                    let current_wd = now.weekday();
                    let mut min_days_ahead = 8u32;
                    for wd in &weekdays {
                        let diff = (*wd as i32 - current_wd as i32).rem_euclid(7) as u32;
                        let days_ahead = if diff == 0 { 7 } else { diff };
                        if days_ahead < min_days_ahead {
                            min_days_ahead = days_ahead;
                        }
                    }

                    // Minutes from now to that day at alarm_time
                    let base_diff = alarm_time - now_time;
                    base_diff.num_minutes() + (min_days_ahead as i64) * 24 * 60
                }
            }
            _ => total_minutes,
        };

        let message = if total_minutes <= 0 {
            fl!("alarm-toast-less-than-minute")
        } else if total_minutes < 60 {
            fl!("alarm-toast-minutes", minutes = total_minutes.to_string())
        } else {
            let hours = total_minutes / 60;
            let mins = total_minutes % 60;
            fl!(
                "alarm-toast-hours-minutes",
                hours = hours.to_string(),
                minutes = mins.to_string()
            )
        };

        self.toasts.push(toaster::Toast::new(message)).map(cosmic::action::app)
    }

    pub(super) fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let mut window_title = fl!("app-title");

        if let Some(page) = self.nav.text(self.nav.active()) {
            window_title.push_str(" — ");
            window_title.push_str(page);
        }

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }
}
