// SPDX-License-Identifier: MIT
//
// Workout update logic: interval state machine and settings handling.

use super::Message;
use super::model::*;
use crate::fl;
use std::time::{Duration, Instant};

impl WorkoutState {
    /// Update and return (notification body, sound) for phase transitions.
    pub fn update(&mut self, message: Message) -> Vec<(String, String)> {
        let mut notifications = Vec::new();

        match message {
            Message::Start(id) => {
                if let Some(w) = self.workouts.iter_mut().find(|w| w.id == id) {
                    w.reset_runtime();
                    w.started = true;
                    w.is_running = true;
                    w.start_instant = Some(Instant::now());
                    w.started_remaining = w.remaining;
                }
            }
            Message::Resume(id) => {
                if let Some(w) = self.workouts.iter_mut().find(|w| w.id == id)
                    && w.phase != Phase::Done
                {
                    w.is_running = true;
                    w.start_instant = Some(Instant::now());
                    w.started_remaining = w.remaining;
                }
            }
            Message::Pause(id) => {
                if let Some(w) = self.workouts.iter_mut().find(|w| w.id == id) {
                    if let Some(start) = w.start_instant.take() {
                        w.remaining = w.started_remaining.saturating_sub(start.elapsed());
                    }
                    w.is_running = false;
                }
            }
            Message::Skip(id) => {
                if let Some(w) = self.workouts.iter_mut().find(|w| w.id == id)
                    && w.started
                {
                    w.advance();
                }
            }
            Message::Reset(id) => {
                if let Some(w) = self.workouts.iter_mut().find(|w| w.id == id) {
                    w.reset_runtime();
                }
            }
            Message::Delete(id) => {
                self.workouts.retain(|w| w.id != id);
            }
            Message::OpenSettings => {
                self.editing_id = None;
                let (prep, work, rest, rounds, sets, set_rest) = Preset::Tabata.config();
                self.edit_label.clear();
                self.edit_prep = prep;
                self.edit_work = work;
                self.edit_rest = rest;
                self.edit_rounds = rounds;
                self.edit_sets = sets;
                self.edit_set_rest = set_rest;
                self.edit_sound = "Bell".to_string();
            }
            Message::StartEditWorkout(id) => {
                if let Some(w) = self.workouts.iter().find(|w| w.id == id) {
                    self.editing_id = Some(id);
                    self.edit_label = w.label.clone();
                    self.edit_prep = w.prep_secs;
                    self.edit_work = w.work_secs;
                    self.edit_rest = w.rest_secs;
                    self.edit_rounds = w.rounds;
                    self.edit_sets = w.sets;
                    self.edit_set_rest = w.set_rest_secs;
                    self.edit_sound = w.sound.clone();
                }
            }
            Message::AddWorkout => {
                let label = if self.edit_label.is_empty() {
                    fl!("workout-default-label", id = self.next_id.to_string())
                } else {
                    self.edit_label.clone()
                };
                self.workouts.push(WorkoutEntry::new(
                    self.next_id,
                    label,
                    self.edit_prep,
                    self.edit_work,
                    self.edit_rest,
                    self.edit_rounds,
                    self.edit_sets,
                    self.edit_set_rest,
                    self.edit_sound.clone(),
                ));
                self.next_id += 1;
                self.edit_label.clear();
            }
            Message::SaveEditWorkout => {
                if let Some(edit_id) = self.editing_id.take()
                    && let Some(w) = self.workouts.iter_mut().find(|w| w.id == edit_id)
                {
                    if !self.edit_label.is_empty() {
                        w.label = self.edit_label.clone();
                    }
                    w.prep_secs = self.edit_prep;
                    w.work_secs = self.edit_work.max(1);
                    w.rest_secs = self.edit_rest;
                    w.rounds = self.edit_rounds.max(1);
                    w.sets = self.edit_sets.max(1);
                    w.set_rest_secs = self.edit_set_rest;
                    w.sound = self.edit_sound.clone();
                    w.reset_runtime();
                }
                self.edit_label.clear();
            }
            Message::CancelEditWorkout => {
                self.editing_id = None;
                self.edit_label.clear();
            }
            Message::EditLabel(label) => {
                self.edit_label = label;
            }
            Message::EditPrep(v) => self.edit_prep = v.min(60),
            Message::EditWork(v) => self.edit_work = v.clamp(1, 600),
            Message::EditRest(v) => self.edit_rest = v.min(600),
            Message::EditRounds(v) => self.edit_rounds = v.clamp(1, 99),
            Message::EditSets(v) => self.edit_sets = v.clamp(1, 99),
            Message::EditSetRest(v) => self.edit_set_rest = v.min(600),
            Message::ApplyPreset(preset) => {
                let (prep, work, rest, rounds, sets, set_rest) = preset.config();
                self.edit_prep = prep;
                self.edit_work = work;
                self.edit_rest = rest;
                self.edit_rounds = rounds;
                self.edit_sets = sets;
                self.edit_set_rest = set_rest;
            }
            Message::EditSound(sound) => {
                self.edit_sound = sound;
            }
            Message::BrowseCustomSound => {
                // Handled in app.rs
            }
            Message::ToggleEditMode => {
                self.edit_mode = !self.edit_mode;
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::StartDrag(index) => {
                self.pre_drag_order = self.workouts.iter().map(|w| w.id).collect();
                self.dragging_index = Some(index);
            }
            Message::Reorder(from, to) => {
                if from < self.workouts.len() && to <= self.workouts.len() && from != to {
                    let item = self.workouts.remove(from);
                    let insert_at = if to > from { to - 1 } else { to };
                    self.workouts.insert(insert_at.min(self.workouts.len()), item);
                    self.dragging_index = Some(insert_at.min(self.workouts.len()));
                }
            }
            Message::FinishDrag => {
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::CancelDrag => {
                if !self.pre_drag_order.is_empty() {
                    let order = &self.pre_drag_order;
                    self.workouts.sort_by_key(|w| {
                        order.iter().position(|&id| id == w.id).unwrap_or(usize::MAX)
                    });
                }
                self.dragging_index = None;
                self.pre_drag_order.clear();
            }
            Message::Tick => {
                for w in &mut self.workouts {
                    if w.is_running
                        && let Some(start) = w.start_instant
                    {
                        w.remaining = w.started_remaining.saturating_sub(start.elapsed());
                        if w.remaining == Duration::ZERO {
                            let new_phase = w.advance();
                            let body = if new_phase == Phase::Done {
                                fl!("workout-complete", label = w.label.clone())
                            } else {
                                fl!(
                                    "workout-phase",
                                    label = w.label.clone(),
                                    phase = new_phase.display_name()
                                )
                            };
                            notifications.push((body, w.sound.clone()));
                        }
                    }
                }
            }
        }

        notifications
    }
}
