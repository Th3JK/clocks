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
                    // A workout with no playable steps is finished on arrival;
                    // starting it would leave a running timer that never ticks.
                    if !w.is_finished() {
                        w.started = true;
                        w.is_running = true;
                        w.start_instant = Some(Instant::now());
                        w.started_remaining = w.remaining;
                    }
                }
            }
            Message::Resume(id) => {
                if let Some(w) = self.workouts.iter_mut().find(|w| w.id == id)
                    && !w.is_finished()
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
                // Never leave focus mode pointing at a deleted workout.
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
                // Only label and sound are loaded. The six steppers describe a
                // simple prep/work/rest layout, which cannot represent an
                // arbitrary block list — so for an existing workout the drawer
                // hides them and structure is edited in the block editor. Seeding
                // them from a guess would silently overwrite custom blocks.
                if let Some(w) = self.workouts.iter().find(|w| w.id == id) {
                    self.editing_id = Some(id);
                    self.edit_label = w.label.clone();
                    self.edit_sound = w.sound.clone();
                }
            }
            Message::AddWorkout => {
                let label = if self.edit_label.is_empty() {
                    fl!("workout-default-label", id = self.next_id.to_string())
                } else {
                    self.edit_label.clone()
                };
                let blocks = simple_blocks(
                    self.edit_prep,
                    self.edit_work,
                    self.edit_rest,
                    self.edit_rounds,
                    self.edit_sets,
                    self.edit_set_rest,
                );
                self.workouts.push(WorkoutEntry::new(
                    self.next_id,
                    label,
                    blocks,
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
                    w.sound = self.edit_sound.clone();
                }
                self.edit_label.clear();
            }
            // --- Block editor (full page) ---
            Message::OpenBlockEditor(id) => {
                if let Some(w) = self.workouts.iter().find(|w| w.id == id) {
                    self.editing_blocks_id = Some(id);
                    // Work on a copy so Cancel is a genuine discard.
                    self.edit_blocks = w.blocks.clone();
                }
            }
            Message::CloseBlockEditor => {
                self.editing_blocks_id = None;
                self.edit_blocks.clear();
            }
            Message::SaveBlocks => {
                if let Some(id) = self.editing_blocks_id.take()
                    && let Some(w) = self.workouts.iter_mut().find(|w| w.id == id)
                {
                    w.blocks = std::mem::take(&mut self.edit_blocks);
                    // Rebuilds the flat plan and rewinds the runtime.
                    w.reset_runtime();
                }
                self.edit_blocks.clear();
            }
            Message::AddBlock => {
                self.edit_blocks.push(Block::new(
                    8,
                    vec![
                        Step::new(String::new(), 30, StepKind::Effort),
                        Step::new(String::new(), 10, StepKind::Recovery),
                    ],
                    true,
                ));
            }
            Message::RemoveBlock(bi) => {
                if bi < self.edit_blocks.len() {
                    self.edit_blocks.remove(bi);
                }
            }
            Message::MoveBlock(bi, delta) => {
                let target = bi as isize + delta;
                if bi < self.edit_blocks.len()
                    && target >= 0
                    && (target as usize) < self.edit_blocks.len()
                {
                    self.edit_blocks.swap(bi, target as usize);
                }
            }
            Message::SetBlockRepeat(bi, v) => {
                if let Some(b) = self.edit_blocks.get_mut(bi) {
                    b.repeat = v.clamp(1, 99);
                }
            }
            Message::ToggleSkipLastRecovery(bi) => {
                if let Some(b) = self.edit_blocks.get_mut(bi) {
                    b.skip_last_recovery = !b.skip_last_recovery;
                }
            }
            Message::AddStep(bi) => {
                if let Some(b) = self.edit_blocks.get_mut(bi) {
                    b.steps
                        .push(Step::new(String::new(), 30, StepKind::Effort));
                }
            }
            Message::RemoveStep(bi, si) => {
                if let Some(b) = self.edit_blocks.get_mut(bi)
                    && si < b.steps.len()
                {
                    b.steps.remove(si);
                }
            }
            Message::MoveStep(bi, si, delta) => {
                if let Some(b) = self.edit_blocks.get_mut(bi) {
                    let target = si as isize + delta;
                    if si < b.steps.len() && target >= 0 && (target as usize) < b.steps.len() {
                        b.steps.swap(si, target as usize);
                    }
                }
            }
            Message::SetStepLabel(bi, si, label) => {
                if let Some(s) = self
                    .edit_blocks
                    .get_mut(bi)
                    .and_then(|b| b.steps.get_mut(si))
                {
                    s.label = label;
                }
            }
            Message::SetStepSecs(bi, si, v) => {
                if let Some(s) = self
                    .edit_blocks
                    .get_mut(bi)
                    .and_then(|b| b.steps.get_mut(si))
                {
                    s.secs = v.min(3600);
                }
            }
            Message::SetStepKind(bi, si, kind) => {
                if let Some(s) = self
                    .edit_blocks
                    .get_mut(bi)
                    .and_then(|b| b.steps.get_mut(si))
                {
                    s.kind = kind;
                }
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
                            // `advance` returns the next step's label, or None
                            // once the plan is exhausted.
                            let body = match w.advance() {
                                Some(step) => fl!(
                                    "workout-phase",
                                    label = w.label.clone(),
                                    phase = step
                                ),
                                None => fl!("workout-complete", label = w.label.clone()),
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
