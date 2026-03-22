// SPDX-License-Identifier: MIT

use crate::components::{format_duration, sound_selector_view};
use crate::fl;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionType {
    Work,
    ShortBreak,
    LongBreak,
}

impl SessionType {
    pub fn display_name(&self) -> String {
        match self {
            SessionType::Work => fl!("session-work"),
            SessionType::ShortBreak => fl!("session-short-break"),
            SessionType::LongBreak => fl!("session-long-break"),
        }
    }
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[derive(Debug, Clone)]
pub struct PomodoroTimer {
    pub id: u32,
    pub label: String,
    pub work_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub session_number: u32,
    pub session_type: SessionType,
    pub remaining: Duration,
    pub is_running: bool,
    pub start_instant: Option<Instant>,
    pub started_remaining: Duration,
    pub completed_work_sessions: u32,
    pub total_focused_secs: u64,
    pub target_sessions: u32,
    pub sound: String,
}

impl PomodoroTimer {
    pub fn from_config(id: u32, label: String, work: u32, short_break: u32, long_break: u32) -> Self {
        Self::new(id, label, work, short_break, long_break)
    }

    fn new(id: u32, label: String, work: u32, short_break: u32, long_break: u32) -> Self {
        let work_dur = Duration::from_secs(work as u64 * 60);
        Self {
            id,
            label,
            work_minutes: work,
            short_break_minutes: short_break,
            long_break_minutes: long_break,
            session_number: 1,
            session_type: SessionType::Work,
            remaining: work_dur,
            is_running: false,
            start_instant: None,
            started_remaining: work_dur,
            completed_work_sessions: 0,
            total_focused_secs: 0,
            target_sessions: 8,
            sound: "Bell".to_string(),
        }
    }

    fn work_duration(&self) -> Duration {
        Duration::from_secs(self.work_minutes as u64 * 60)
    }

    fn short_break_duration(&self) -> Duration {
        Duration::from_secs(self.short_break_minutes as u64 * 60)
    }

    fn long_break_duration(&self) -> Duration {
        Duration::from_secs(self.long_break_minutes as u64 * 60)
    }

    fn advance_session(&mut self) {
        match self.session_type {
            SessionType::Work => {
                self.completed_work_sessions += 1;
                self.total_focused_secs += self.work_minutes as u64 * 60;
                if self.completed_work_sessions % 4 == 0 {
                    self.session_type = SessionType::LongBreak;
                    self.remaining = self.long_break_duration();
                } else {
                    self.session_type = SessionType::ShortBreak;
                    self.remaining = self.short_break_duration();
                }
            }
            SessionType::ShortBreak | SessionType::LongBreak => {
                self.session_number += 1;
                self.session_type = SessionType::Work;
                self.remaining = self.work_duration();
            }
        }
        self.started_remaining = self.remaining;
        self.start_instant = Some(Instant::now());
    }
}

pub struct PomodoroState {
    pub timers: Vec<PomodoroTimer>,
    pub next_id: u32,
    // Settings defaults for new timers
    pub default_work_minutes: u32,
    pub default_short_break_minutes: u32,
    pub default_long_break_minutes: u32,
    // Editing state for new/existing timer
    pub edit_label: String,
    pub editing_id: Option<u32>,
    pub edit_work_minutes: u32,
    pub edit_short_break_minutes: u32,
    pub edit_long_break_minutes: u32,
    pub edit_sound: String,
}

impl Default for PomodoroState {
    fn default() -> Self {
        let mut state = Self {
            timers: Vec::new(),
            next_id: 1,
            default_work_minutes: 25,
            default_short_break_minutes: 5,
            default_long_break_minutes: 15,
            edit_label: String::new(),
            editing_id: None,
            edit_work_minutes: 25,
            edit_short_break_minutes: 5,
            edit_long_break_minutes: 15,
            edit_sound: "Bell".to_string(),
        };
        // Create a default pomodoro timer
        state
            .timers
            .push(PomodoroTimer::new(0, "Pomodoro".to_string(), 25, 5, 15));
        state
    }
}

impl PomodoroState {
    pub fn is_running(&self) -> bool {
        self.timers.iter().any(|t| t.is_running)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Start(u32),
    Pause(u32),
    Resume(u32),
    Skip(u32),
    Reset(u32),
    Delete(u32),
    // Settings sidebar
    OpenSettings,
    AddTimer,
    StartEditPomodoro(u32),
    EditNewLabel(String),
    SetDefaultWorkMinutes(u32),
    SetDefaultShortBreakMinutes(u32),
    SetDefaultLongBreakMinutes(u32),
    SaveEditPomodoro,
    CancelEditPomodoro,
    EditSound(String),
    BrowseCustomSound,
    Tick,
}

impl PomodoroState {
    /// Update and return list of (description, sound) for session transitions
    pub fn update(&mut self, message: Message) -> Vec<(String, String)> {
        let mut notifications = Vec::new();

        match message {
            Message::OpenSettings => {
                // Handled in app.rs
            }
            Message::Start(id) => {
                if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                    timer.is_running = true;
                    timer.start_instant = Some(Instant::now());
                    timer.started_remaining = timer.remaining;
                }
            }
            Message::Pause(id) => {
                if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                    if let Some(start) = timer.start_instant.take() {
                        timer.remaining = timer.started_remaining.saturating_sub(start.elapsed());
                    }
                    timer.is_running = false;
                }
            }
            Message::Resume(id) => {
                if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                    timer.is_running = true;
                    timer.start_instant = Some(Instant::now());
                    timer.started_remaining = timer.remaining;
                }
            }
            Message::Skip(id) => {
                if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                    let was_running = timer.is_running;
                    timer.advance_session();
                    if !was_running {
                        timer.start_instant = None;
                        timer.is_running = false;
                    }
                }
            }
            Message::Reset(id) => {
                if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                    let label = timer.label.clone();
                    let w = timer.work_minutes;
                    let s = timer.short_break_minutes;
                    let l = timer.long_break_minutes;
                    let tid = timer.id;
                    *timer = PomodoroTimer::new(tid, label, w, s, l);
                }
            }
            Message::Delete(id) => {
                self.timers.retain(|t| t.id != id);
            }
            Message::AddTimer => {
                let label = if self.edit_label.is_empty() {
                    fl!("pomodoro-default-label", id = self.next_id.to_string())
                } else {
                    self.edit_label.clone()
                };
                self.timers.push(PomodoroTimer::new(
                    self.next_id,
                    label,
                    self.default_work_minutes,
                    self.default_short_break_minutes,
                    self.default_long_break_minutes,
                ));
                self.next_id += 1;
                self.edit_label.clear();
            }
            Message::StartEditPomodoro(id) => {
                if let Some(timer) = self.timers.iter().find(|t| t.id == id) {
                    self.editing_id = Some(id);
                    self.edit_label = timer.label.clone();
                    self.edit_work_minutes = timer.work_minutes;
                    self.edit_short_break_minutes = timer.short_break_minutes;
                    self.edit_long_break_minutes = timer.long_break_minutes;
                    self.edit_sound = timer.sound.clone();
                }
            }
            Message::EditSound(sound) => {
                self.edit_sound = sound;
            }
            Message::BrowseCustomSound => {
                // Handled in app.rs
            }
            Message::SaveEditPomodoro => {
                if let Some(edit_id) = self.editing_id.take() {
                    if let Some(timer) = self.timers.iter_mut().find(|t| t.id == edit_id) {
                        if !self.edit_label.is_empty() {
                            timer.label = self.edit_label.clone();
                        }
                        timer.work_minutes = self.edit_work_minutes;
                        timer.short_break_minutes = self.edit_short_break_minutes;
                        timer.long_break_minutes = self.edit_long_break_minutes;
                        timer.sound = self.edit_sound.clone();
                        // Reset duration based on current session type
                        match timer.session_type {
                            SessionType::Work => {
                                timer.remaining = timer.work_duration();
                                timer.started_remaining = timer.remaining;
                            }
                            SessionType::ShortBreak => {
                                timer.remaining = timer.short_break_duration();
                                timer.started_remaining = timer.remaining;
                            }
                            SessionType::LongBreak => {
                                timer.remaining = timer.long_break_duration();
                                timer.started_remaining = timer.remaining;
                            }
                        }
                        timer.is_running = false;
                        timer.start_instant = None;
                    }
                }
                self.edit_label.clear();
            }
            Message::CancelEditPomodoro => {
                self.editing_id = None;
                self.edit_label.clear();
            }
            Message::EditNewLabel(label) => {
                self.edit_label = label;
            }
            Message::SetDefaultWorkMinutes(m) => {
                let val = m.max(1).min(120);
                if self.editing_id.is_some() {
                    self.edit_work_minutes = val;
                } else {
                    self.default_work_minutes = val;
                }
            }
            Message::SetDefaultShortBreakMinutes(m) => {
                let val = m.max(1).min(60);
                if self.editing_id.is_some() {
                    self.edit_short_break_minutes = val;
                } else {
                    self.default_short_break_minutes = val;
                }
            }
            Message::SetDefaultLongBreakMinutes(m) => {
                let val = m.max(1).min(60);
                if self.editing_id.is_some() {
                    self.edit_long_break_minutes = val;
                } else {
                    self.default_long_break_minutes = val;
                }
            }
            Message::Tick => {
                for timer in &mut self.timers {
                    if timer.is_running {
                        if let Some(start) = timer.start_instant {
                            timer.remaining =
                                timer.started_remaining.saturating_sub(start.elapsed());
                            if timer.remaining == Duration::ZERO {
                                let prev_type = timer.session_type;
                                timer.advance_session();
                                notifications.push((
                                    fl!("pomodoro-transition",
                                        label = timer.label.clone(),
                                        prev = prev_type.display_name(),
                                        next = timer.session_type.display_name()
                                    ),
                                    timer.sound.clone(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        notifications
    }

    /// Main view: page header + all pomodoro timers
    pub fn view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(self.timers.len() * 5 + 2).spacing(spacing);

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("pomodoro-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .tooltip(fl!("tooltip-add"))
                    .on_press(Message::OpenSettings),
            );
        col = col.push(header);

        if self.timers.is_empty() {
            col = col.push(
                widget::container(widget::text::body(fl!("no-pomodoro-timers")))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .padding(24),
            );
        }

        for timer in &self.timers {
            let id = timer.id;

            col = col.push(widget::divider::horizontal::default());

            // Label row with Edit/Delete aligned right
            let mut label_row = widget::row::with_capacity(3)
                .spacing(spacing)
                .align_y(Alignment::Center)
                .push(
                    widget::text::title4(&timer.label).width(Length::Fill),
                );
            if !timer.is_running {
                label_row = label_row.push(
                    widget::button::icon(widget::icon::from_name("edit-symbolic"))
                        .tooltip(fl!("tooltip-edit"))
                        .on_press(Message::StartEditPomodoro(id)),
                );
            }
            label_row = label_row.push(
                widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                    .tooltip(fl!("tooltip-delete"))
                    .on_press(Message::Delete(id)),
            );
            col = col.push(label_row);

            // Session info
            let session_info = fl!("session-info",
                number = timer.session_number.to_string(),
                session_type = timer.session_type.display_name()
            );
            col = col.push(widget::text::caption(session_info));

            // Time display
            col = col.push(
                widget::container(widget::text::title1(format_duration(timer.remaining)))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .padding(12),
            );

            // Controls (without Edit/Delete, those are now in the label row)
            let mut controls = widget::row::with_capacity(3)
                .spacing(spacing)
                .align_y(Alignment::Center);

            if timer.is_running {
                controls =
                    controls.push(widget::button::standard(fl!("pause")).on_press(Message::Pause(id)));
            } else if timer.remaining < timer.started_remaining
                || (timer.completed_work_sessions > 0 && timer.session_type == SessionType::Work)
            {
                controls = controls
                    .push(widget::button::suggested(fl!("resume")).on_press(Message::Resume(id)));
            } else {
                controls =
                    controls.push(widget::button::suggested(fl!("start")).on_press(Message::Start(id)));
            }

            controls = controls.push(widget::button::standard(fl!("skip")).on_press(Message::Skip(id)));
            controls =
                controls.push(widget::button::destructive(fl!("reset")).on_press(Message::Reset(id)));

            col = col.push(
                widget::container(controls)
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            );

            // Progress
            let progress_text = fl!("progress-info",
                completed = timer.completed_work_sessions.to_string(),
                target = timer.target_sessions.to_string(),
                focused = (timer.total_focused_secs / 60).to_string()
            );
            col = col.push(
                widget::container(widget::text::caption(progress_text))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            );
        }

        col.into()
    }

    /// Settings sidebar view
    pub fn settings_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(14).spacing(spacing);

        if let Some(_edit_id) = self.editing_id {
            // Editing existing pomodoro timer
            col = col.push(widget::text::title4(fl!("edit-pomodoro")));
            col = col.push(
                widget::text_input(fl!("label"), &self.edit_label).on_input(Message::EditNewLabel),
            );

            let w = self.edit_work_minutes;
            let work_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("work-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultWorkMinutes(w.saturating_sub(5))),
                )
                .push(widget::text::body(fl!("minutes-value", value = w.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultWorkMinutes(w + 5)),
                );
            col = col.push(work_row);

            let sb = self.edit_short_break_minutes;
            let short_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("short-break-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultShortBreakMinutes(sb.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = sb.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultShortBreakMinutes(sb + 1)),
                );
            col = col.push(short_row);

            let lb = self.edit_long_break_minutes;
            let long_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("long-break-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultLongBreakMinutes(lb.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = lb.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultLongBreakMinutes(lb + 1)),
                );
            col = col.push(long_row);

            // Sound selection
            col = col.push(widget::divider::horizontal::default());
            col = col.push(sound_selector_view(
                fl!("sound"),
                &self.edit_sound,
                Message::EditSound,
                Message::BrowseCustomSound,
            ));

            col = col.push(widget::divider::horizontal::default());
            let actions = widget::row::with_capacity(2)
                .spacing(8)
                .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEditPomodoro))
                .push(widget::button::suggested(fl!("save")).on_press(Message::SaveEditPomodoro));
            col = col.push(actions);
        } else {
            // Add new timer
            col = col.push(widget::text::title4(fl!("new-pomodoro")));
            col = col.push(
                widget::text_input(fl!("label-placeholder-pomodoro"), &self.edit_label)
                    .on_input(Message::EditNewLabel),
            );
            col = col.push(widget::button::suggested(fl!("add-timer")).on_press(Message::AddTimer));

            col = col.push(widget::divider::horizontal::default());

            // Default durations for new timers
            col = col.push(widget::text::title4(fl!("default-durations")));

            let w = self.default_work_minutes;
            let work_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("work-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultWorkMinutes(w.saturating_sub(5))),
                )
                .push(widget::text::body(fl!("minutes-value", value = w.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultWorkMinutes(w + 5)),
                );
            col = col.push(work_row);

            let sb = self.default_short_break_minutes;
            let short_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("short-break-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultShortBreakMinutes(sb.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = sb.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultShortBreakMinutes(sb + 1)),
                );
            col = col.push(short_row);

            let lb = self.default_long_break_minutes;
            let long_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("long-break-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultLongBreakMinutes(lb.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = lb.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultLongBreakMinutes(lb + 1)),
                );
            col = col.push(long_row);
        }

        col.into()
    }
}
