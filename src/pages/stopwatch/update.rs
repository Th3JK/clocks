// SPDX-License-Identifier: MIT
//
// Stopwatch update logic: start, stop, lap, reset, and history management.

use super::model::*;
use super::Message;
use crate::fl;
use std::time::{Duration, Instant};

impl StopwatchState {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Start => {
                self.is_running = true;
                self.start_instant = Some(Instant::now());
                // Create history entry for new session if none exists
                if self.current_session_id.is_none() {
                    let id = self.next_history_id;
                    self.next_history_id += 1;
                    self.current_session_id = Some(id);
                    self.history.push(StopwatchRecord {
                        id,
                        label: if self.current_label.is_empty() {
                            fl!("session-default", id = id.to_string())
                        } else {
                            self.current_label.clone()
                        },
                        total_elapsed: Duration::ZERO,
                        laps: Vec::new(),
                    });
                }
            }
            Message::Stop => {
                if let Some(start) = self.start_instant.take() {
                    self.accumulated += start.elapsed();
                }
                self.is_running = false;
                self.elapsed = self.accumulated;
                // Update current session
                if let Some(session_id) = self.current_session_id
                    && let Some(record) = self.history.iter_mut().find(|r| r.id == session_id)
                {
                    record.total_elapsed = self.elapsed;
                    record.laps = self.laps.clone();
                }
            }
            Message::Reset => {
                // Finalize current session in history
                if let Some(session_id) = self.current_session_id {
                    let total = self.current_elapsed();
                    if let Some(record) = self.history.iter_mut().find(|r| r.id == session_id) {
                        record.total_elapsed = total;
                        record.laps = self.laps.clone();
                    }
                    // Remove the record if nothing was recorded
                    if total == Duration::ZERO {
                        self.history.retain(|r| r.id != session_id);
                    }
                }
                // Reset stopwatch state but keep history
                let history = std::mem::take(&mut self.history);
                let next_history_id = self.next_history_id;
                *self = Self::default();
                self.history = history;
                self.next_history_id = next_history_id;
            }
            Message::Lap => {
                let current_elapsed = self.current_elapsed();
                let lap_time = current_elapsed.saturating_sub(self.lap_start);

                let delta = if let Some(prev) = self.laps.last() {
                    lap_time.as_millis() as i64 - prev.lap_time.as_millis() as i64
                } else {
                    0
                };

                self.laps.push(LapEntry {
                    id: self.next_lap_id,
                    lap_time,
                    delta,
                    is_fastest: false,
                    is_slowest: false,
                });
                self.next_lap_id += 1;
                self.lap_start = current_elapsed;

                if self.laps.len() >= 2 {
                    let min = self.laps.iter().map(|l| l.lap_time).min().unwrap();
                    let max = self.laps.iter().map(|l| l.lap_time).max().unwrap();
                    for lap in &mut self.laps {
                        lap.is_fastest = lap.lap_time == min;
                        lap.is_slowest = lap.lap_time == max;
                    }
                }
                // Update current session history entry
                if let Some(session_id) = self.current_session_id
                    && let Some(record) = self.history.iter_mut().find(|r| r.id == session_id)
                {
                    record.total_elapsed = current_elapsed;
                    record.laps = self.laps.clone();
                }
            }
            Message::Tick => {
                self.elapsed = self.current_elapsed();
            }
            Message::EditHistoryLabel(id, label) => {
                if let Some(record) = self.history.iter_mut().find(|r| r.id == id) {
                    record.label = label;
                }
            }
            Message::DeleteHistory(id) => {
                self.history.retain(|r| r.id != id);
            }
            Message::ResumeFromHistory(id) => {
                if let Some(record) = self.history.iter().find(|r| r.id == id) {
                    self.accumulated = record.total_elapsed;
                    self.elapsed = record.total_elapsed;
                    self.laps = record.laps.clone();
                    self.next_lap_id = self.laps.len() as u32 + 1;
                    self.lap_start = record.total_elapsed;
                    self.is_running = false;
                    self.start_instant = None;
                    self.current_label = record.label.clone();
                    self.current_session_id = Some(id);
                }
            }
            Message::ClearHistory => {
                self.history.clear();
            }
            Message::OpenHistory => {
                // Handled in app.rs
            }
        }
    }
}
