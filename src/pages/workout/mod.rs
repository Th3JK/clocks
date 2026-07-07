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
    // Edit mode (reorder/delete)
    ToggleEditMode,
    StartDrag(usize),
    Reorder(usize, usize),
    FinishDrag,
    CancelDrag,
}
