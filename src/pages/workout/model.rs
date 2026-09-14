// SPDX-License-Identifier: MIT
//
// Workout interval-timer data types.
//
// A workout is an ordered list of `Block`s. Each block holds an ordered list of
// `Step`s and a repeat count, and one repetition plays every step in order — so
// `Block { repeat: 8, steps: [Shadow boxing 60s, Rest 10s] }` is eight rounds of
// "60s work, 10s rest", not eight minutes of work followed by eighty seconds of
// rest.
//
// At runtime the blocks are flattened once into a `Vec<ResolvedStep>` and the
// entry just walks an index. That keeps the tick trivial, makes total duration
// and overall progress free, and removes the termination hazard the previous
// on-the-fly phase machine had with zero-length phases.

use crate::fl;
use std::time::{Duration, Instant};

/// What a step is for. Drives the ring colour and which trailing step is elided
/// on a block's final repetition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Prep,
    Effort,
    Recovery,
}

impl StepKind {
    /// Default label for a step of this kind, used by generated workouts and as
    /// the placeholder for an unnamed step.
    pub fn default_label(self) -> String {
        match self {
            StepKind::Prep => fl!("workout-prep"),
            StepKind::Effort => fl!("workout-work"),
            StepKind::Recovery => fl!("workout-rest"),
        }
    }

    pub fn display_name(self) -> String {
        match self {
            StepKind::Prep => fl!("workout-kind-prep"),
            StepKind::Effort => fl!("workout-kind-effort"),
            StepKind::Recovery => fl!("workout-kind-recovery"),
        }
    }

    pub const ALL: [StepKind; 3] = [StepKind::Prep, StepKind::Effort, StepKind::Recovery];
}

/// One timed step within a block.
#[derive(Debug, Clone)]
pub struct Step {
    pub label: String,
    pub secs: u32,
    pub kind: StepKind,
}

impl Step {
    pub fn new(label: impl Into<String>, secs: u32, kind: StepKind) -> Self {
        Self {
            label: label.into(),
            secs,
            kind,
        }
    }

    /// The label to show, falling back to the kind's default when unnamed.
    pub fn display_label(&self) -> String {
        if self.label.trim().is_empty() {
            self.kind.default_label()
        } else {
            self.label.clone()
        }
    }
}

/// A repeated group of steps.
#[derive(Debug, Clone)]
pub struct Block {
    pub repeat: u32,
    pub steps: Vec<Step>,
    /// Drop the block's trailing recovery step(s) on the final repetition, so a
    /// block doesn't end on a rest before moving to the next one.
    pub skip_last_recovery: bool,
}

impl Block {
    pub fn new(repeat: u32, steps: Vec<Step>, skip_last_recovery: bool) -> Self {
        Self {
            repeat: repeat.max(1),
            steps,
            skip_last_recovery,
        }
    }

    /// How many `Recovery` steps sit at the end of the step list. Those are the
    /// ones elided on the final repetition.
    fn trailing_recovery(&self) -> usize {
        self.steps
            .iter()
            .rev()
            .take_while(|s| s.kind == StepKind::Recovery)
            .count()
    }
}

/// A single step occurrence in the flattened plan, carrying the position
/// information the status line needs.
#[derive(Debug, Clone)]
pub struct ResolvedStep {
    pub label: String,
    pub secs: u32,
    pub kind: StepKind,
    pub block: usize,
    pub blocks: usize,
    pub rep: u32,
    pub reps: u32,
}

/// Expand blocks into the flat sequence actually played.
///
/// Zero-second steps are dropped here rather than skipped at runtime: that is
/// what lets `advance()` be a plain index bump, and it means a plan can never
/// contain a step that would spin the advance loop forever.
pub fn flatten(blocks: &[Block]) -> Vec<ResolvedStep> {
    let total_blocks = blocks.len();
    let mut plan = Vec::new();

    for (bi, block) in blocks.iter().enumerate() {
        let trailing = if block.skip_last_recovery {
            block.trailing_recovery()
        } else {
            0
        };

        for rep in 1..=block.repeat {
            let last_rep = rep == block.repeat;
            let take = if last_rep {
                block.steps.len().saturating_sub(trailing)
            } else {
                block.steps.len()
            };

            for step in block.steps.iter().take(take) {
                if step.secs == 0 {
                    continue;
                }
                plan.push(ResolvedStep {
                    label: step.display_label(),
                    secs: step.secs,
                    kind: step.kind,
                    block: bi + 1,
                    blocks: total_blocks,
                    rep,
                    reps: block.repeat,
                });
            }
        }
    }

    plan
}

/// Build the block list for a classic prep / work / rest / rounds / sets layout.
///
/// Used both by the presets and by the migration of workouts saved before blocks
/// existed. A flat block list cannot nest, so `sets` expands to `2 * sets - 1`
/// blocks (the set bodies, with a rest block between each pair). Both shipped
/// presets use `sets == 1`, so the common case stays a single block.
pub fn simple_blocks(
    prep: u32,
    work: u32,
    rest: u32,
    rounds: u32,
    sets: u32,
    set_rest: u32,
) -> Vec<Block> {
    let mut blocks = Vec::new();

    if prep > 0 {
        blocks.push(Block::new(
            1,
            vec![Step::new(String::new(), prep, StepKind::Prep)],
            false,
        ));
    }

    let sets = sets.max(1);
    for set in 1..=sets {
        blocks.push(Block::new(
            rounds.max(1),
            vec![
                Step::new(String::new(), work.max(1), StepKind::Effort),
                Step::new(String::new(), rest, StepKind::Recovery),
            ],
            true,
        ));
        if set < sets && set_rest > 0 {
            blocks.push(Block::new(
                1,
                vec![Step::new(
                    fl!("workout-set-rest"),
                    set_rest,
                    StepKind::Recovery,
                )],
                false,
            ));
        }
    }

    blocks
}

/// A named preset selectable in the settings sidebar.
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

    pub fn blocks(self) -> Vec<Block> {
        let (prep, work, rest, rounds, sets, set_rest) = self.config();
        simple_blocks(prep, work, rest, rounds, sets, set_rest)
    }
}

#[derive(Debug, Clone)]
pub struct WorkoutEntry {
    pub id: u32,
    pub label: String,
    pub blocks: Vec<Block>,
    pub sound: String,
    // Derived from `blocks`; rebuilt on construction, reset, and save.
    pub plan: Vec<ResolvedStep>,
    // Runtime
    pub cursor: usize,
    pub finished: bool,
    pub remaining: Duration,
    pub is_running: bool,
    pub started: bool,
    pub start_instant: Option<Instant>,
    pub started_remaining: Duration,
}

impl WorkoutEntry {
    pub fn new(id: u32, label: String, blocks: Vec<Block>, sound: String) -> Self {
        let mut entry = Self {
            id,
            label,
            blocks,
            sound,
            plan: Vec::new(),
            cursor: 0,
            finished: false,
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

    /// Recompute the flat plan and rewind to the pristine, never-started state.
    pub(super) fn reset_runtime(&mut self) {
        self.plan = flatten(&self.blocks);
        self.cursor = 0;
        self.is_running = false;
        self.started = false;
        self.start_instant = None;
        // An empty plan (no blocks, or every step zero-length) is immediately
        // finished rather than a workout that can be started but never advances.
        self.finished = self.plan.is_empty();
        let remaining = self
            .plan
            .first()
            .map(|s| Self::dur(s.secs))
            .unwrap_or(Duration::ZERO);
        self.remaining = remaining;
        self.started_remaining = remaining;
    }

    pub fn current_step(&self) -> Option<&ResolvedStep> {
        self.plan.get(self.cursor)
    }

    /// Duration of the step in progress, for the progress ring. Never zero, so
    /// it is safe as a denominator.
    pub(super) fn step_total(&self) -> Duration {
        Self::dur(self.current_step().map(|s| s.secs).unwrap_or(1).max(1))
    }

    /// Total length of the whole workout.
    pub fn total_duration(&self) -> Duration {
        Self::dur(self.plan.iter().map(|s| s.secs).sum())
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// True when the step in progress is effort (drives the accent colour).
    pub fn is_effort(&self) -> bool {
        self.current_step()
            .map(|s| s.kind == StepKind::Effort)
            .unwrap_or(false)
    }

    pub(super) fn has_started(&self) -> bool {
        self.started
    }

    /// Advance to the next step, returning its label (or `None` once finished).
    /// Re-anchors the wall clock exactly as the previous engine did.
    pub(super) fn advance(&mut self) -> Option<String> {
        self.cursor += 1;
        if self.cursor >= self.plan.len() {
            self.cursor = self.plan.len();
            self.finished = true;
            self.is_running = false;
            self.start_instant = None;
            self.remaining = Duration::ZERO;
            self.started_remaining = Duration::ZERO;
            return None;
        }

        let step = &self.plan[self.cursor];
        let label = step.label.clone();
        self.remaining = Self::dur(step.secs);
        self.started_remaining = self.remaining;
        if self.is_running {
            self.start_instant = Some(Instant::now());
        }
        Some(label)
    }
}

pub struct WorkoutState {
    pub workouts: Vec<WorkoutEntry>,
    pub next_id: u32,
    // Simple-form editing state (context drawer)
    pub editing_id: Option<u32>,
    pub edit_label: String,
    pub edit_prep: u32,
    pub edit_work: u32,
    pub edit_rest: u32,
    pub edit_rounds: u32,
    pub edit_sets: u32,
    pub edit_set_rest: u32,
    pub edit_sound: String,
    // Block editing state (full page). `edit_blocks` is a working copy so that
    // cancelling leaves the saved workout untouched.
    pub editing_blocks_id: Option<u32>,
    pub edit_blocks: Vec<Block>,
    // Edit mode (reorder/delete)
    pub edit_mode: bool,
    pub dragging_index: Option<usize>,
    pub pre_drag_order: Vec<u32>,
    /// Workout shown full-page in focus mode. Session-only, not persisted.
    pub focused_id: Option<u32>,
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
            editing_blocks_id: None,
            edit_blocks: Vec::new(),
            edit_mode: false,
            dragging_index: None,
            pre_drag_order: Vec::new(),
            focused_id: None,
        };
        // Seed Tabata and HIIT presets as ready-to-run workouts.
        for (preset, label) in [
            (Preset::Tabata, fl!("workout-preset-tabata")),
            (Preset::Hiit, fl!("workout-preset-hiit")),
        ] {
            let id = state.next_id;
            state
                .workouts
                .push(WorkoutEntry::new(id, label, preset.blocks(), "Bell".to_string()));
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
