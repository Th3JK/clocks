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
    // Settings sidebar
    OpenSettings,
    AddTimer,
    StartEditPomodoro(u32),
    EditNewLabel(String),
    SetDefaultWorkMinutes(u32),
    SetDefaultShortBreakMinutes(u32),
    SetDefaultLongBreakMinutes(u32),
    SaveEditPomodoro,
    CancelEditPomodoro,
    EditSound(String),
    BrowseCustomSound,
    Tick,
}
