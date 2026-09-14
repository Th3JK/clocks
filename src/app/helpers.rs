// SPDX-License-Identifier: MIT
//
// Private helper methods on `AppModel`: tick handling, page shortcuts,
// state persistence, audio control, and title updates.

use super::persistence::build_config_from_state;
use super::{AppModel, Message};
use crate::audio;
use crate::fl;
use crate::pages::{
    Page, alarm, chess, countdown, pomodoro, stopwatch, timer, workout, world_clocks,
};
use cosmic_config::CosmicConfigEntry;
use chrono::{Datelike, Local, NaiveTime, Offset, TimeZone, Utc};
use cosmic::prelude::*;
use cosmic::widget::{self, toaster};

impl AppModel {
    /// Central tick handler: drives stopwatch, timers, pomodoro, and alarm logic
    pub(super) fn handle_tick(&mut self) {
        // Stopwatch tick
        if self.stopwatch.is_running {
            self.stopwatch.update(stopwatch::Message::Tick);
        }

        // Timers: display only. The daemon owns the countdown and fires the
        // notification, so this just recomputes what is on screen from the
        // deadlines it published -- no `Instant`, and nothing to duplicate.
        if !self.runtime.timers.is_empty() {
            let now = Local::now();
            for entry in &mut self.timer.timers {
                if let Some(run) = self.runtime.timer(entry.id) {
                    entry.is_running = run.is_running();
                    entry.remaining = std::time::Duration::from_secs(run.remaining_secs(now));
                    entry.completed_count = run.completed;
                }
            }
        }

        // Pomodoro: display only, like timers. The daemon advances the session
        // cycle and fires the notification.
        if !self.runtime.pomodoro.is_empty() {
            let now = Local::now();
            for entry in &mut self.pomodoro.timers {
                if let Some(run) = self.runtime.pomodoro(entry.id) {
                    entry.is_running = run.is_running();
                    entry.remaining = std::time::Duration::from_secs(run.remaining_secs(now));
                }
            }
        }

        // Chess clock tick + flag notification
        if self.chess.is_running() {
            let notifications = self.chess.update(chess::Message::Tick);
            for msg in notifications {
                audio::send_notification(&fl!("notification-chess"), &msg);
                audio::play_sound("Bell");
            }
        }

        // Workout tick + phase-change notifications
        if self.workout.has_running() {
            let notifications = self.workout.update(workout::Message::Tick);
            for (msg, sound) in notifications {
                audio::send_notification(&fl!("notification-workout"), &msg);
                audio::play_sound(&sound);
            }
        }

        // Countdown: the daemon delivers reminders and arrivals. This is only
        // the yearly roll-forward, which rewrites a definition field and so has
        // to stay on this side.
        if self.countdown.has_pending() {
            self.countdown.update(countdown::Message::Tick);
        }

        // Alarms are deliberately absent: `clocks-daemon` owns them. It fires
        // whether or not this window exists, which is the entire point of the
        // split, and duplicating the schedule here would ring twice whenever
        // both processes are up. Ringing state arrives through the runtime
        // config entry instead -- see `Message::UpdateRuntime`.
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
            &self.stopwatch,
            &self.chess,
            &self.workout,
            &self.countdown,
            &self.nav_order,
            &self.nav_hidden,
            self.time_format,
            self.confirm_delete_alarm,
            self.confirm_delete_timer,
            self.confirm_delete_world_clock,
            self.confirm_delete_pomodoro,
            self.confirm_clear_stopwatch,
            self.auto_sort_alarms,
            self.auto_sort_world_clocks,
            self.auto_clear_stopwatch_history,
        );
        if let Err(e) = config.write_entry(ctx) {
            eprintln!("Failed to save config: {:?}", e);
        }
    }

    /// Rebuild the sidebar from the stored order and visibility.
    ///
    /// The nav is cleared and repopulated rather than mutated in place, because
    /// `Model::remove` bumps the slotmap generation — any `Entity` held across
    /// the call is silently stale, so entities must never be persisted or
    /// cached. Pages are looked up by `Page`, which is stable.
    pub(super) fn rebuild_nav(&mut self) {
        // Remember the active page, not its entity: the entity will not survive.
        let previously_active = self.nav.active_data::<Page>().copied();

        self.nav.clear();
        for page in &self.nav_order {
            if self.nav_hidden.contains(page) {
                continue;
            }
            self.nav
                .insert()
                .text(page_title(*page))
                .data::<Page>(*page)
                .icon(page_icon(*page));
        }

        // `clear` deactivates everything. Leaving it that way makes `view()`
        // fall through to "select a view", so always land somewhere: the page
        // that was active if it is still visible, otherwise the first one.
        let target = previously_active
            .filter(|p| !self.nav_hidden.contains(p))
            .and_then(|p| {
                self.nav
                    .iter()
                    .find(|e| self.nav.data::<Page>(*e) == Some(&p))
            })
            .or_else(|| self.nav.iter().next());
        if let Some(entity) = target {
            self.nav.activate(entity);
        }
    }

    /// Jump to a page by identity rather than position, so this keeps working
    /// if the sidebar order ever becomes user-defined.
    pub(super) fn activate_page(&mut self, page: Page) {
        let target = self
            .nav
            .iter()
            .find(|e| self.nav.data::<Page>(*e) == Some(&page));
        if let Some(entity) = target {
            self.nav.activate(entity);
        }
    }

    /// Rows offered in the palette, before filtering.
    ///
    /// The user's own saved items come first — those are what gets reached for
    /// repeatedly — followed by the built-in durations and every page.
    pub(super) fn palette_suggestions(&self) -> Vec<crate::quick_action::QuickAction> {
        use crate::quick_action::QuickAction;
        let mut out: Vec<QuickAction> = Vec::new();
        out.extend(self.timer.timers.iter().map(|t| QuickAction::StartTimer(t.id)));
        out.extend(
            self.pomodoro
                .timers
                .iter()
                .map(|p| QuickAction::StartPomodoro(p.id)),
        );
        out.extend(
            self.workout
                .workouts
                .iter()
                .map(|w| QuickAction::StartWorkout(w.id)),
        );
        out.extend(crate::quick_action::presets());
        out
    }

    /// Carry out a parsed quick action.
    ///
    /// Page `update()` methods are called directly rather than routed through
    /// `Message::<Page>(..)`: the app-level arms for `StartNew`/`OpenSettings`
    /// open the context drawer and move focus, which is wrong for something
    /// invoked from the palette.
    pub(super) fn run_quick_action(
        &mut self,
        action: crate::quick_action::QuickAction,
    ) -> Task<cosmic::Action<Message>> {
        use crate::quick_action::QuickAction;

        match action {
            QuickAction::Timer { secs, label } => {
                // `StartNew` is the only thing that clears `edit_id`; without it
                // a previous edit would make `SaveTimer` overwrite that timer.
                self.timer.update(timer::Message::StartNew);
                self.timer
                    .update(timer::Message::EditHours((secs / 3600) as u8));
                self.timer
                    .update(timer::Message::EditMinutes(((secs % 3600) / 60) as u8));
                self.timer
                    .update(timer::Message::EditSeconds((secs % 60) as u8));
                if let Some(label) = label {
                    self.timer.update(timer::Message::EditLabel(label));
                }
                self.timer.update(timer::Message::SaveTimer);
                // Quick actions are a "do it now" gesture, so start it running.
                if let Some(id) = self.timer.timers.last().map(|t| t.id) {
                    self.active_timer_id = Some(id);
                    self.timer.update(timer::Message::StartTimer(id));
                }
                self.activate_page(Page::Timer);
            }

            QuickAction::Alarm {
                hour,
                minute,
                label,
            } => {
                // The alarm page has no SetHour/SetMinute message, only
                // increment/decrement, so the edit buffer is written directly.
                // `SaveAlarm` converts through `hour12_to_24` only in 12h mode,
                // so the stored hour has to match the active format.
                let (edit_hour, is_pm) = if self.use_12h {
                    let (h, pm) = crate::time_format::to_12h(hour);
                    (h as u8, pm)
                } else {
                    (hour as u8, false)
                };
                self.alarm.editing = Some(alarm::AlarmEdit {
                    id: None,
                    hour: edit_hour,
                    minute: minute as u8,
                    is_pm,
                    label: label.unwrap_or_default(),
                    repeat_mode: alarm::RepeatMode::Once,
                    sound: "Bell".to_string(),
                    snooze_minutes: 5,
                    ring_minutes: 1,
                });
                self.alarm
                    .update(alarm::Message::SaveAlarm, self.use_12h);
                self.activate_page(Page::Alarm);
                // Reuse the normal "rings in X" toast.
                if let Some(alarm) = self
                    .alarm
                    .last_saved_id
                    .and_then(|id| self.alarm.alarms.iter().find(|a| a.id == id))
                    .cloned()
                {
                    let task = self.push_alarm_toast(&alarm);
                    self.save_state();
                    return task;
                }
            }

            QuickAction::Countdown {
                year,
                month,
                day,
                label,
            } => {
                self.countdown.update(countdown::Message::OpenSettings);
                self.countdown
                    .update(countdown::Message::EditDate(year, month, day));
                if let Some(label) = label {
                    self.countdown.update(countdown::Message::EditLabel(label));
                }
                self.countdown.update(countdown::Message::AddEvent);
                self.activate_page(Page::Countdown);
            }

            QuickAction::Clock { query } => {
                // Same matching the world-clocks search uses, so the palette and
                // the sidebar agree on what a city name means.
                let q = query.to_lowercase();
                let found = chrono_tz::TZ_VARIANTS.iter().find(|tz| {
                    tz.name().to_lowercase().contains(&q)
                        || world_clocks::tz_city_name(**tz).to_lowercase().contains(&q)
                });
                if let Some(tz) = found {
                    self.world_clocks
                        .update(world_clocks::Message::AddClock(*tz));
                }
                self.activate_page(Page::WorldClocks);
            }

            QuickAction::Navigate(page) => {
                self.activate_page(page);
            }

            // Launchers for saved items. Each starts from the top, which is what
            // "start" means from a palette — resuming is the card's job.
            QuickAction::StartTimer(id) => {
                if self.timer.timers.iter().any(|t| t.id == id) {
                    self.active_timer_id = Some(id);
                    self.timer.update(timer::Message::ResetTimer(id));
                    self.timer.update(timer::Message::StartTimer(id));
                    self.activate_page(Page::Timer);
                }
            }
            QuickAction::StartPomodoro(id) => {
                if self.pomodoro.timers.iter().any(|p| p.id == id) {
                    self.active_pomodoro_id = Some(id);
                    self.pomodoro.update(pomodoro::Message::Start(id));
                    self.activate_page(Page::Pomodoro);
                }
            }
            QuickAction::StartWorkout(id) => {
                if self.workout.workouts.iter().any(|w| w.id == id) {
                    self.workout.update(workout::Message::Start(id));
                    self.activate_page(Page::Workout);
                }
            }
        }

        self.save_state();
        self.update_title()
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
            Some(Page::Chess) => {
                // Space acts as the clock tap: starts the game if idle, otherwise
                // commits the running clock and hands over to the opponent.
                let player = self.chess.current_turn;
                self.chess.update(chess::Message::TapPlayer(player));
                self.save_state();
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
            Some(Page::Chess) => {
                self.chess.update(chess::Message::Reset);
                self.save_state();
            }
            _ => {}
        }
        Task::none()
    }

    pub(super) fn handle_page_shortcut_ctrl_n(&mut self) -> Task<cosmic::Action<Message>> {
        match self.nav.active_data::<Page>() {
            Some(Page::Countdown) => {
                self.countdown.update(countdown::Message::OpenSettings);
                self.context_page = crate::pages::ContextPage::CountdownEdit;
                self.core.window.show_context = true;
                self.save_state();
                return widget::text_input::focus(widget::Id::new("countdown-label-input"));
            }
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

/// Nav label for a page.
pub(super) fn page_title(page: Page) -> String {
    match page {
        Page::WorldClocks => fl!("nav-world-clocks"),
        Page::Stopwatch => fl!("nav-stopwatch"),
        Page::Alarm => fl!("nav-alarm"),
        Page::Timer => fl!("nav-timer"),
        Page::Pomodoro => fl!("nav-pomodoro"),
        Page::Chess => fl!("nav-chess"),
        Page::Workout => fl!("nav-workout"),
        Page::Countdown => fl!("nav-countdown"),
    }
}

/// Nav icon for a page. Timer, Pomodoro and Countdown are bundled because no
/// system glyph distinguishes them.
pub(super) fn page_icon(page: Page) -> widget::icon::Icon {
    match page {
        Page::WorldClocks => widget::icon::from_name("preferences-system-time-symbolic").icon(),
        Page::Stopwatch => widget::icon::from_name("media-playback-start-symbolic").icon(),
        Page::Alarm => widget::icon::from_name("alarm-symbolic").icon(),
        Page::Timer => widget::icon::icon(super::bundled_icon(super::TIMER_ICON)),
        Page::Pomodoro => widget::icon::icon(super::bundled_icon(super::POMODORO_ICON)),
        Page::Chess => widget::icon::from_name("view-grid-symbolic").icon(),
        Page::Workout => widget::icon::from_name("emblem-favorite-symbolic").icon(),
        Page::Countdown => widget::icon::icon(super::bundled_icon(super::COUNTDOWN_ICON)),
    }
}
