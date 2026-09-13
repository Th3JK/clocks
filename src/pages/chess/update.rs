// SPDX-License-Identifier: MIT
//
// Chess clock update logic.

use super::Message;
use super::model::*;
use crate::fl;
use std::time::Duration;

impl ChessState {
    /// Update and return notification messages (e.g. a flag/time-out) to surface.
    pub fn update(&mut self, message: Message) -> Vec<String> {
        let mut notifications = Vec::new();

        match message {
            Message::TapPlayer(player) => {
                // Only the player to move may end their turn, and only while the
                // game isn't over.
                if self.flagged.is_none() && self.current_turn == player {
                    if self.running {
                        // Capture how long the move took before `commit_active_clock`
                        // consumes `start_instant`. Only counted when the clock was
                        // actually running, so the initial tap isn't a "move".
                        let elapsed =
                            self.start_instant.map(|s| s.elapsed()).unwrap_or_default();
                        self.commit_active_clock();
                        self.add_increment(player);
                        self.record_move(player, elapsed);
                    }
                    self.current_turn = player.opponent();
                    self.running = true;
                    self.start_active_clock();
                }
            }
            Message::PauseToggle => {
                if self.flagged.is_none() {
                    if self.running {
                        self.commit_active_clock();
                        self.running = false;
                    } else {
                        self.running = true;
                        self.start_active_clock();
                    }
                }
            }
            Message::Reset => {
                *self = ChessState::new(self.base_minutes, self.increment_secs);
            }
            Message::OpenSettings => {
                self.edit_base_minutes = self.base_minutes;
                self.edit_increment_secs = self.increment_secs;
            }
            Message::EditBaseMinutes(m) => {
                self.edit_base_minutes = m.clamp(1, 180);
            }
            Message::EditIncrementSecs(s) => {
                self.edit_increment_secs = s.min(60);
            }
            Message::ApplyPreset(base, inc) => {
                self.edit_base_minutes = base.clamp(1, 180);
                self.edit_increment_secs = inc.min(60);
            }
            Message::ApplySettings => {
                self.base_minutes = self.edit_base_minutes;
                self.increment_secs = self.edit_increment_secs;
                *self = ChessState::new(self.base_minutes, self.increment_secs);
            }
            Message::Tick => {
                if self.running
                    && let Some(start) = self.start_instant
                {
                    let remaining = self
                        .active_started_remaining
                        .saturating_sub(start.elapsed());
                    self.set_remaining(self.current_turn, remaining);
                    if remaining == Duration::ZERO {
                        self.flagged = Some(self.current_turn);
                        self.running = false;
                        self.start_instant = None;
                        notifications.push(fl!(
                            "chess-flagged",
                            player = self.current_turn.display_name()
                        ));
                    }
                }
            }
        }

        notifications
    }
}
