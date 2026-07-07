// SPDX-License-Identifier: MIT
//
// Workout (HIIT/Tabata) interval-timer data types.

use crate::fl;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Prep,
    Work,
    Rest,
    SetRest,
    Done,
}

impl Phase {
    pub fn display_name(self) -> String {
        match self {
            Phase::Prep => fl!("workout-prep"),
            Phase::Work => fl!("workout-work"),
            Phase::Rest => fl!("workout-rest"),
            Phase::SetRest => fl!("workout-set-rest"),
            Phase::Done => fl!("workout-done"),
        }
    }
}

/// A named interval-timer preset selectable in the settings sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Tabata,
    Hiit,
}

impl Preset {
    /// (prep, work, rest, rounds, sets, set_rest) in seconds/counts.
    pub fn config(self) -> (u32, u32, u32, u32, u32, u32) {
        match self {
            Preset::Tabata => (10, 20, 10, 8, 1, 60),
            Preset::Hiit => (10, 40, 20, 8, 1, 60),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkoutEntry {
    pub id: u32,
    pub label: String,
    pub prep_secs: u32,
    pub work_secs: u32,
    pub rest_secs: u32,
    pub rounds: u32,
    pub sets: u32,
    pub set_rest_secs: u32,
    pub sound: String,
    // Runtime
    pub phase: Phase,
    pub current_round: u32,
    pub current_set: u32,
    pub remaining: Duration,
    pub is_running: bool,
    pub started: bool,
    pub start_instant: Option<Instant>,
    pub started_remaining: Duration,
}

impl WorkoutEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u32,
        label: String,
        prep_secs: u32,
        work_secs: u32,
        rest_secs: u32,
        rounds: u32,
        sets: u32,
        set_rest_secs: u32,
        sound: String,
    ) -> Self {
        let mut entry = Self {
            id,
            label,
            prep_secs,
            work_secs: work_secs.max(1),
            rest_secs,
            rounds: rounds.max(1),
            sets: sets.max(1),
            set_rest_secs,
            sound,
            phase: Phase::Prep,
            current_round: 1,
            current_set: 1,
            remaining: Duration::ZERO,
            is_running: false,
            started: false,
            start_instant: None,
            started_remaining: Duration::ZERO,
        };
        entry.reset_runtime();
        entry
    }

    fn dur(secs: u32) -> Duration {
        Duration::from_secs(secs as u64)
    }

    /// Reset the runtime to the pristine, never-started state.
    pub(super) fn reset_runtime(&mut self) {
        self.current_round = 1;
        self.current_set = 1;
        self.is_running = false;
        self.started = false;
        self.start_instant = None;
        let (phase, remaining) = self.initial_phase();
        self.phase = phase;
        self.remaining = remaining;
        self.started_remaining = remaining;
    }

    /// The phase the workout begins in (skips a zero-length prep).
    fn initial_phase(&self) -> (Phase, Duration) {
        if self.prep_secs > 0 {
            (Phase::Prep, Self::dur(self.prep_secs))
        } else {
            (Phase::Work, Self::dur(self.work_secs))
        }
    }

    /// Total duration of the current phase (for the progress ring).
    pub(super) fn phase_total(&self) -> Duration {
        let secs = match self.phase {
            Phase::Prep => self.prep_secs,
            Phase::Work => self.work_secs,
            Phase::Rest => self.rest_secs,
            Phase::SetRest => self.set_rest_secs,
            Phase::Done => 1,
        };
        Self::dur(secs.max(1))
    }

    pub(super) fn has_started(&self) -> bool {
        self.started
    }

    /// Perform a single phase transition (without skipping zero-length phases).
    fn advance_phase_once(&mut self) {
        match self.phase {
            Phase::Prep => {
                self.phase = Phase::Work;
                self.remaining = Self::dur(self.work_secs);
            }
            Phase::Work => {
                if self.current_round < self.rounds {
                    self.phase = Phase::Rest;
                    self.remaining = Self::dur(self.rest_secs);
                } else if self.current_set < self.sets {
                    self.phase = Phase::SetRest;
                    self.remaining = Self::dur(self.set_rest_secs);
                } else {
                    self.phase = Phase::Done;
                    self.remaining = Duration::ZERO;
                }
            }
            Phase::Rest => {
                self.current_round += 1;
                self.phase = Phase::Work;
                self.remaining = Self::dur(self.work_secs);
            }
            Phase::SetRest => {
                self.current_set += 1;
                self.current_round = 1;
                self.phase = Phase::Work;
                self.remaining = Self::dur(self.work_secs);
            }
            Phase::Done => {}
        }
    }

    /// Advance to the next phase, skipping any zero-length phases. Returns the new
    /// phase. When the workout finishes it stops running.
    pub(super) fn advance(&mut self) -> Phase {
        loop {
            self.advance_phase_once();
            if self.phase == Phase::Done {
                self.is_running = false;
                self.start_instant = None;
                break;
            }
            if self.remaining > Duration::ZERO {
                break;
            }
        }
        self.started_remaining = self.remaining;
        if self.is_running {
            self.start_instant = Some(Instant::now());
        }
        self.phase
    }
}

pub struct WorkoutState {
    pub workouts: Vec<WorkoutEntry>,
    pub next_id: u32,
    // Editing state
    pub editing_id: Option<u32>,
    pub edit_label: String,
    pub edit_prep: u32,
    pub edit_work: u32,
    pub edit_rest: u32,
    pub edit_rounds: u32,
    pub edit_sets: u32,
    pub edit_set_rest: u32,
    pub edit_sound: String,
    // Edit mode (reorder/delete)
    pub edit_mode: bool,
    pub dragging_index: Option<usize>,
    pub pre_drag_order: Vec<u32>,
}

impl Default for WorkoutState {
    fn default() -> Self {
        let (pp, pw, pr, prd, ps, psr) = Preset::Tabata.config();
        let mut state = Self {
            workouts: Vec::new(),
            next_id: 1,
            editing_id: None,
            edit_label: String::new(),
            edit_prep: pp,
            edit_work: pw,
            edit_rest: pr,
            edit_rounds: prd,
            edit_sets: ps,
            edit_set_rest: psr,
            edit_sound: "Bell".to_string(),
            edit_mode: false,
            dragging_index: None,
            pre_drag_order: Vec::new(),
        };
        // Seed Tabata and HIIT presets as ready-to-run workouts.
        for (preset, label_key) in [
            (Preset::Tabata, "workout-preset-tabata"),
            (Preset::Hiit, "workout-preset-hiit"),
        ] {
            let (prep, work, rest, rounds, sets, set_rest) = preset.config();
            let id = state.next_id;
            state.workouts.push(WorkoutEntry::new(
                id,
                fl_label(label_key),
                prep,
                work,
                rest,
                rounds,
                sets,
                set_rest,
                "Bell".to_string(),
            ));
            state.next_id += 1;
        }
        state
    }
}

impl WorkoutState {
    pub fn has_running(&self) -> bool {
        self.workouts.iter().any(|w| w.is_running)
    }
}

fn fl_label(key: &str) -> String {
    match key {
        "workout-preset-tabata" => fl!("workout-preset-tabata"),
        "workout-preset-hiit" => fl!("workout-preset-hiit"),
        _ => key.to_string(),
    }
}
