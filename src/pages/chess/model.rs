// SPDX-License-Identifier: MIT
//
// Chess clock data types: players and the two-clock game state.

use crate::fl;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    White,
    Black,
}

impl Player {
    pub fn opponent(self) -> Player {
        match self {
            Player::White => Player::Black,
            Player::Black => Player::White,
        }
    }

    pub fn display_name(self) -> String {
        match self {
            Player::White => fl!("chess-white"),
            Player::Black => fl!("chess-black"),
        }
    }
}

pub struct ChessState {
    pub white_remaining: Duration,
    pub black_remaining: Duration,
    /// Whose clock runs while the game is running.
    pub current_turn: Player,
    /// Whether a clock is currently counting down.
    pub running: bool,
    /// Player who ran out of time (lost on time), if any.
    pub flagged: Option<Player>,
    /// Baseline for the active clock (set when it started ticking).
    pub start_instant: Option<Instant>,
    pub active_started_remaining: Duration,
    // Configuration
    pub base_minutes: u32,
    pub increment_secs: u32,
    // Settings edit fields
    pub edit_base_minutes: u32,
    pub edit_increment_secs: u32,
}

impl Default for ChessState {
    fn default() -> Self {
        Self::new(5, 0)
    }
}

impl ChessState {
    pub fn new(base_minutes: u32, increment_secs: u32) -> Self {
        let base = Duration::from_secs(base_minutes as u64 * 60);
        Self {
            white_remaining: base,
            black_remaining: base,
            current_turn: Player::White,
            running: false,
            flagged: None,
            start_instant: None,
            active_started_remaining: base,
            base_minutes,
            increment_secs,
            edit_base_minutes: base_minutes,
            edit_increment_secs: increment_secs,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn remaining_of(&self, player: Player) -> Duration {
        match player {
            Player::White => self.white_remaining,
            Player::Black => self.black_remaining,
        }
    }

    pub(super) fn set_remaining(&mut self, player: Player, value: Duration) {
        match player {
            Player::White => self.white_remaining = value,
            Player::Black => self.black_remaining = value,
        }
    }

    pub(super) fn add_increment(&mut self, player: Player) {
        let inc = Duration::from_secs(self.increment_secs as u64);
        let current = self.remaining_of(player);
        self.set_remaining(player, current + inc);
    }

    /// Begin counting down `current_turn`'s clock from now.
    pub(super) fn start_active_clock(&mut self) {
        self.start_instant = Some(Instant::now());
        self.active_started_remaining = self.remaining_of(self.current_turn);
    }

    /// Commit the elapsed time of the running clock back into its remaining value.
    pub(super) fn commit_active_clock(&mut self) {
        if let Some(start) = self.start_instant.take() {
            let remaining = self
                .active_started_remaining
                .saturating_sub(start.elapsed());
            self.set_remaining(self.current_turn, remaining);
        }
    }
}
