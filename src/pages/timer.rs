// SPDX-License-Identifier: MIT

use crate::components::{format_duration_hms, sound_selector_view};
use crate::fl;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct TimerEntry {
    pub id: u32,
    pub label: String,
    pub initial_duration: Duration,
    pub remaining: Duration,
    pub is_running: bool,
    pub start_instant: Option<Instant>,
    pub started_remaining: Duration,
    pub repeat_enabled: bool,
    pub repeat_count: u32, // 0 = infinite
    pub completed_count: u32,
    pub sound: String,
}

pub struct TimerState {
    pub timers: Vec<TimerEntry>,
    pub next_id: u32,
    // Timer editing state
    pub editing: bool,
    pub edit_id: Option<u32>, // None = new, Some(id) = editing existing
    pub edit_hours: u8,
    pub edit_minutes: u8,
    pub edit_seconds: u8,
    pub edit_label: String,
    pub edit_repeat: bool,
    pub edit_repeat_count: u32,
    pub edit_sound: String,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            timers: Vec::new(),
            next_id: 1,
            editing: false,
            edit_id: None,
            edit_hours: 0,
            edit_minutes: 5,
            edit_seconds: 0,
            edit_label: String::new(),
            edit_repeat: false,
            edit_repeat_count: 1,
            edit_sound: "Bell".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    StartNew,
    StartEditTimer(u32),
    CancelEdit,
    SaveTimer,
    EditLabel(String),
    EditHours(u8),
    EditMinutes(u8),
    EditSeconds(u8),
    ToggleEditRepeat,
    EditRepeatCount(u32),
    EditSound(String),
    StartTimer(u32),
    PauseTimer(u32),
    ResumeTimer(u32),
    ResetTimer(u32),
    DeleteTimer(u32),
    BrowseCustomSound,
    Tick,
}

impl TimerState {
    /// Update and return list of (label, sound) for timers that just completed
    pub fn update(&mut self, message: Message) -> Vec<(String, String)> {
        let mut completed_labels = Vec::new();

        match message {
            Message::StartNew => {
                self.editing = true;
                self.edit_id = None;
                self.edit_hours = 0;
                self.edit_minutes = 5;
                self.edit_seconds = 0;
                self.edit_label.clear();
                self.edit_repeat = false;
                self.edit_repeat_count = 1;
                self.edit_sound = "Bell".to_string();
            }
            Message::StartEditTimer(id) => {
                if let Some(timer) = self.timers.iter().find(|t| t.id == id) {
                    self.editing = true;
                    self.edit_id = Some(id);
                    let total_secs = timer.initial_duration.as_secs();
                    self.edit_hours = (total_secs / 3600) as u8;
                    self.edit_minutes = ((total_secs % 3600) / 60) as u8;
                    self.edit_seconds = (total_secs % 60) as u8;
                    self.edit_label = timer.label.clone();
                    self.edit_repeat = timer.repeat_enabled;
                    self.edit_repeat_count = timer.repeat_count;
                    self.edit_sound = timer.sound.clone();
                }
            }
            Message::CancelEdit => {
                self.editing = false;
            }
            Message::SaveTimer => {
                let dur = Duration::from_secs(
                    self.edit_hours as u64 * 3600
                        + self.edit_minutes as u64 * 60
                        + self.edit_seconds as u64,
                );
                if dur > Duration::ZERO {
                    let label = if self.edit_label.is_empty() {
                        if let Some(id) = self.edit_id {
                            fl!("timer-default-label", id = id.to_string())
                        } else {
                            fl!("timer-default-label", id = self.next_id.to_string())
                        }
                    } else {
                        self.edit_label.clone()
                    };
                    let repeat_count = if self.edit_repeat {
                        self.edit_repeat_count
                    } else {
                        0
                    };

                    if let Some(edit_id) = self.edit_id {
                        // Update existing timer
                        if let Some(timer) = self.timers.iter_mut().find(|t| t.id == edit_id) {
                            timer.label = label;
                            timer.initial_duration = dur;
                            timer.remaining = dur;
                            timer.started_remaining = dur;
                            timer.is_running = false;
                            timer.start_instant = None;
                            timer.repeat_enabled = self.edit_repeat;
                            timer.repeat_count = repeat_count;
                            timer.completed_count = 0;
                            timer.sound = self.edit_sound.clone();
                        }
                    } else {
                        // Create new timer
                        self.timers.push(TimerEntry {
                            id: self.next_id,
                            label,
                            initial_duration: dur,
                            remaining: dur,
                            is_running: false,
                            start_instant: None,
                            started_remaining: dur,
                            repeat_enabled: self.edit_repeat,
                            repeat_count: repeat_count,
                            completed_count: 0,
                            sound: self.edit_sound.clone(),
                        });
                        self.next_id += 1;
                    }
                    self.editing = false;
                }
            }
            Message::EditLabel(label) => {
                self.edit_label = label;
            }
            // Wrap-around time picker
            Message::EditHours(h) => {
                self.edit_hours = h % 24;
            }
            Message::EditMinutes(m) => {
                self.edit_minutes = m % 60;
            }
            Message::EditSeconds(s) => {
                self.edit_seconds = s % 60;
            }
            Message::ToggleEditRepeat => {
                self.edit_repeat = !self.edit_repeat;
            }
            Message::EditRepeatCount(c) => {
                self.edit_repeat_count = c;
            }
            Message::EditSound(sound) => {
                self.edit_sound = sound;
            }
            Message::BrowseCustomSound => {
                // Handled in app.rs
            }
            Message::StartTimer(id) => {
                if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                    timer.is_running = true;
                    timer.start_instant = Some(Instant::now());
                    timer.started_remaining = timer.remaining;
                }
            }
            Message::PauseTimer(id) => {
                if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                    if let Some(start) = timer.start_instant.take() {
                        let elapsed = start.elapsed();
                        timer.remaining = timer.started_remaining.saturating_sub(elapsed);
                    }
                    timer.is_running = false;
                }
            }
            Message::ResumeTimer(id) => {
                if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                    timer.is_running = true;
                    timer.start_instant = Some(Instant::now());
                    timer.started_remaining = timer.remaining;
                }
            }
            Message::ResetTimer(id) => {
                if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
                    timer.remaining = timer.initial_duration;
                    timer.started_remaining = timer.initial_duration;
                    timer.is_running = false;
                    timer.start_instant = None;
                    timer.completed_count = 0;
                }
            }
            Message::DeleteTimer(id) => {
                self.timers.retain(|t| t.id != id);
            }
            Message::Tick => {
                for timer in &mut self.timers {
                    if timer.is_running {
                        if let Some(start) = timer.start_instant {
                            let elapsed = start.elapsed();
                            timer.remaining = timer.started_remaining.saturating_sub(elapsed);

                            if timer.remaining == Duration::ZERO {
                                timer.completed_count += 1;
                                completed_labels.push((timer.label.clone(), timer.sound.clone()));

                                if timer.repeat_enabled
                                    && (timer.repeat_count == 0
                                        || timer.completed_count < timer.repeat_count)
                                {
                                    // Restart
                                    timer.remaining = timer.initial_duration;
                                    timer.started_remaining = timer.initial_duration;
                                    timer.start_instant = Some(Instant::now());
                                } else {
                                    timer.is_running = false;
                                    timer.start_instant = None;
                                }
                            }
                        }
                    }
                }
            }
        }

        completed_labels
    }

    pub fn has_running_timers(&self) -> bool {
        self.timers.iter().any(|t| t.is_running)
    }

    /// Main view: page header + timer list
    pub fn view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(self.timers.len() + 3).spacing(spacing);

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("timer-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .tooltip(fl!("tooltip-add"))
                    .on_press(Message::StartNew),
            );
        col = col.push(header);

        if self.timers.is_empty() {
            col = col.push(
                widget::container(widget::text::body(fl!("no-timers")))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .padding(24),
            );
        }

        for timer in &self.timers {
            let remaining_str = format_duration_hms(timer.remaining);
            let id = timer.id;

            let mut row = widget::row::with_capacity(4)
                .spacing(spacing)
                .align_y(Alignment::Center);

            let mut info_col = widget::column::with_capacity(2);
            info_col = info_col.push(widget::text::body(&timer.label));
            if timer.repeat_enabled {
                let repeat_str = if timer.repeat_count == 0 {
                    fl!("repeat-progress-infinite", completed = timer.completed_count.to_string())
                } else {
                    fl!("repeat-progress", completed = timer.completed_count.to_string(), total = timer.repeat_count.to_string())
                };
                info_col = info_col.push(widget::text::caption(repeat_str));
            }

            row = row.push(info_col.width(Length::Fill));
            row = row.push(widget::text::title3(remaining_str));

            // Control buttons in a themed container
            let mut ctrl_row = widget::row::with_capacity(2)
                .spacing(4)
                .align_y(Alignment::Center);

            if timer.is_running {
                ctrl_row = ctrl_row.push(
                    widget::button::icon(widget::icon::from_name("media-playback-stop-symbolic"))
                        .tooltip(fl!("tooltip-pause"))
                        .on_press(Message::PauseTimer(id)),
                );
            } else if timer.remaining > Duration::ZERO {
                ctrl_row = ctrl_row.push(
                    widget::button::icon(widget::icon::from_name("media-playback-start-symbolic"))
                        .tooltip(fl!("tooltip-start"))
                        .on_press(if timer.remaining < timer.initial_duration {
                            Message::ResumeTimer(id)
                        } else {
                            Message::StartTimer(id)
                        }),
                );
            } else {
                ctrl_row = ctrl_row.push(
                    widget::icon::from_name("object-select-symbolic").size(24),
                );
            }

            ctrl_row = ctrl_row.push(
                widget::button::icon(widget::icon::from_name("edit-undo-symbolic"))
                    .tooltip(fl!("tooltip-reset"))
                    .on_press(Message::ResetTimer(id)),
            );

            row = row.push(ctrl_row);

            if !timer.is_running {
                row = row.push(
                    widget::button::icon(widget::icon::from_name("edit-symbolic"))
                        .tooltip(fl!("tooltip-edit"))
                        .on_press(Message::StartEditTimer(id)),
                );
            }
            row = row.push(
                widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                    .tooltip(fl!("tooltip-delete"))
                    .on_press(Message::DeleteTimer(id)),
            );

            col = col.push(row);
        }

        col.into()
    }

    /// Sidebar view: timer creation form
    pub fn sidebar_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(8).spacing(spacing);

        // Label
        col = col.push(widget::text::body(fl!("label")));
        col = col
            .push(
                widget::text_input(fl!("timer-label-placeholder"), &self.edit_label)
                    .id(widget::Id::new("timer-label-input"))
                    .on_input(Message::EditLabel),
            );

        // Duration spinners with wrap-around (HH:MM:SS colon format)
        col = col.push(widget::text::body(fl!("duration")));

        let h = self.edit_hours;
        let m = self.edit_minutes;
        let s = self.edit_seconds;

        let dur_row = widget::row::with_capacity(11)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                    .on_press(Message::EditHours(if h == 0 { 23 } else { h - 1 })),
            )
            .push(widget::text::title3(format!("{:02}", h)))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                    .on_press(Message::EditHours((h + 1) % 24)),
            )
            .push(widget::text::title3(":"))
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                    .on_press(Message::EditMinutes(if m == 0 { 59 } else { m - 1 })),
            )
            .push(widget::text::title3(format!("{:02}", m)))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                    .on_press(Message::EditMinutes((m + 1) % 60)),
            )
            .push(widget::text::title3(":"))
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                    .on_press(Message::EditSeconds(if s == 0 { 59 } else { s - 1 })),
            )
            .push(widget::text::title3(format!("{:02}", s)))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                    .on_press(Message::EditSeconds((s + 1) % 60)),
            );
        col = col.push(dur_row);

        // Repeat toggle
        col = col.push(widget::text::body(fl!("repeat")));
        let repeat_btn = if self.edit_repeat {
            widget::button::suggested(fl!("repeat-on")).on_press(Message::ToggleEditRepeat)
        } else {
            widget::button::standard(fl!("repeat-off")).on_press(Message::ToggleEditRepeat)
        };
        col = col.push(repeat_btn);

        // Repeat count (when repeat is on)
        if self.edit_repeat {
            let c = self.edit_repeat_count;
            let count_label = if c == 0 {
                "∞".to_string()
            } else {
                format!("{}", c)
            };

            let count_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("repeat-count")))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
    
                        .on_press(Message::EditRepeatCount(c.saturating_sub(1))),
                )
                .push(widget::text::title4(count_label))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))
    
                        .on_press(Message::EditRepeatCount(c + 1)),
                );
            col = col.push(count_row);
            col = col.push(widget::text::caption(fl!("infinite-repeats")));
        }

        // Sound selection
        col = col.push(widget::divider::horizontal::default());
        col = col.push(sound_selector_view(
            fl!("sound"),
            &self.edit_sound,
            Message::EditSound,
            Message::BrowseCustomSound,
        ));

        // Actions
        col = col.push(widget::divider::horizontal::default());
        let save_label = if self.edit_id.is_some() {
            fl!("save")
        } else {
            fl!("add-timer")
        };
        let actions = widget::row::with_capacity(2)
            .spacing(8)
            .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEdit))
            .push(widget::button::suggested(save_label).on_press(Message::SaveTimer));
        col = col.push(actions);

        col.into()
    }
}
