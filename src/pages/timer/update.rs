// SPDX-License-Identifier: MIT
//
// Timer update logic: message handling, countdown, and repeat logic.

use super::model::*;
use super::Message;
use crate::fl;
use std::time::{Duration, Instant};

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
                            repeat_count,
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
                // Never leave focus mode pointing at a deleted timer.
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
                    self.timers
                        .insert(insert_at.min(self.timers.len()), item);
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
                for timer in &mut self.timers {
                    if timer.is_running
                        && let Some(start) = timer.start_instant
                    {
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

        completed_labels
    }

    pub fn has_running_timers(&self) -> bool {
        self.timers.iter().any(|t| t.is_running)
    }
}
