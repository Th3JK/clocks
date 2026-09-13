// SPDX-License-Identifier: MIT

mod model;
mod update;
mod view;

pub use model::*;

#[derive(Debug, Clone)]
pub enum Message {
    Start(u32),
    Pause(u32),
    Resume(u32),
    Skip(u32),
    Reset(u32),
    Delete(u32),
    Tick,
    // Focus mode (single workout, full page)
    Focus(u32),
    Unfocus,
    // Settings sidebar
    OpenSettings,
    StartEditWorkout(u32),
    AddWorkout,
    SaveEditWorkout,
    CancelEditWorkout,
    EditLabel(String),
    EditPrep(u32),
    EditWork(u32),
    EditRest(u32),
    EditRounds(u32),
    EditSets(u32),
    EditSetRest(u32),
    ApplyPreset(Preset),
    EditSound(String),
    BrowseCustomSound,
    // Block editor (full page). Indices address `edit_blocks`; `isize` deltas
    // are -1/+1 for move up/down.
    OpenBlockEditor(u32),
    CloseBlockEditor,
    SaveBlocks,
    AddBlock,
    RemoveBlock(usize),
    MoveBlock(usize, isize),
    SetBlockRepeat(usize, u32),
    ToggleSkipLastRecovery(usize),
    AddStep(usize),
    RemoveStep(usize, usize),
    MoveStep(usize, usize, isize),
    SetStepLabel(usize, usize, String),
    SetStepSecs(usize, usize, u32),
    SetStepKind(usize, usize, StepKind),
    // Edit mode (reorder/delete)
    ToggleEditMode,
    StartDrag(usize),
    Reorder(usize, usize),
    FinishDrag,
    CancelDrag,
}
