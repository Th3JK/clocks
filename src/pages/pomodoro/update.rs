// SPDX-License-Identifier: MIT
//
// Pomodoro update logic: message handling, session transitions, and timer control.

use super::model::*;
use super::Message;
use crate::fl;
use std::time::{Duration, Instant};

impl PomodoroState {
    /// Update and return list of (description, sound) for session transitions
    pub fn update(&mut self, message: Message) -> Vec<(String, String)> {
        let mut notifications = Vec::new();

        match message {
            Message::OpenSettings => {
                // Opening the drawer always means "create new". Without clearing
                // `editing_id` here, opening it after an edit (e.g. dismissing the
                // drawer with the header X, which touches no page state) reopens
                // that timer's edit form, and saving silently overwrites it.
                self.editing_id = None;
                self.edit_label.clear();
                self.edit_work_minutes = self.default_work_minutes;
                self.edit_short_break_minutes = self.default_short_break_minutes;
                self.edit_long_break_minutes = self.default_long_break_minutes;
                self.edit_sound = "Bell".to_string();
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
                // Don't leave the drawer bound to a timer that no longer exists.
                if self.editing_id == Some(id) {
                    self.editing_id = None;
                    self.edit_label.clear();
                }
                // Nor focus mode.
                if self.focused_id == Some(id) {
                    self.focused_id = None;
                }
            }
            Message::Focus(id) => {
                self.focused_id = Some(id);
            }
            Message::Unfocus => {
                self.focused_id = None;
            }
            Message::AddTimer => {
                let label = if self.edit_label.is_empty() {
                    fl!("pomodoro-default-label", id = self.next_id.to_string())
                } else {
                    self.edit_label.clone()
                };
                let mut timer = PomodoroTimer::new(
                    self.next_id,
                    label,
                    self.default_work_minutes,
                    self.default_short_break_minutes,
                    self.default_long_break_minutes,
                );
                // `PomodoroTimer::new` hardcodes "Bell"; honour the chosen sound.
                timer.sound = self.edit_sound.clone();
                self.timers.push(timer);
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
                if let Some(edit_id) = self.editing_id.take()
                    && let Some(timer) = self.timers.iter_mut().find(|t| t.id == edit_id)
                {
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
                let val = m.clamp(1, 120);
                if self.editing_id.is_some() {
                    self.edit_work_minutes = val;
                } else {
                    self.default_work_minutes = val;
                }
            }
            Message::SetDefaultShortBreakMinutes(m) => {
                let val = m.clamp(1, 60);
                if self.editing_id.is_some() {
                    self.edit_short_break_minutes = val;
                } else {
                    self.default_short_break_minutes = val;
                }
            }
            Message::SetDefaultLongBreakMinutes(m) => {
                let val = m.clamp(1, 60);
                if self.editing_id.is_some() {
                    self.edit_long_break_minutes = val;
                } else {
                    self.default_long_break_minutes = val;
                }
            }
            Message::ToggleEditMode => {
                self.edit_mode = !self.edit_mode;
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::StartDrag(index) => {
                self.pre_drag_order = self.timers.iter().map(|t| t.id).collect();
                self.dragging_index = Some(index);
            }
            Message::Reorder(from, to) => {
                if from < self.timers.len() && to <= self.timers.len() && from != to {
                    let item = self.timers.remove(from);
                    let insert_at = if to > from { to - 1 } else { to };
                    self.timers.insert(insert_at.min(self.timers.len()), item);
                    self.dragging_index = Some(insert_at.min(self.timers.len()));
                }
            }
            Message::FinishDrag => {
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::CancelDrag => {
                if !self.pre_drag_order.is_empty() {
                    let order = &self.pre_drag_order;
                    self.timers.sort_by_key(|t| {
                        order.iter().position(|&id| id == t.id).unwrap_or(usize::MAX)
                    });
                }
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::Tick => {
                // Focus seconds to record after the loop (avoids a second &mut self borrow).
                let mut completed_work_secs: Vec<u64> = Vec::new();
                for timer in &mut self.timers {
                    if timer.is_running
                        && let Some(start) = timer.start_instant
                    {
                        timer.remaining =
                            timer.started_remaining.saturating_sub(start.elapsed());
                        if timer.remaining == Duration::ZERO {
                            let prev_type = timer.session_type;
                            if prev_type == SessionType::Work {
                                completed_work_secs.push(timer.work_minutes as u64 * 60);
                            }
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
                for secs in completed_work_secs {
                    self.record_completed_work(secs);
                }
            }
        }

        notifications
    }
}
