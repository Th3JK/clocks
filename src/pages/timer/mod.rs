// SPDX-License-Identifier: MIT

mod model;
mod update;
mod view;

pub use model::*;

#[derive(Debug, Clone)]
pub enum Message {
    StartNew,
    StartEditTimer(u32),
    CancelEdit,
    SaveTimer,
    EditLabel(String),
    EditHours(u8),
    EditMinutes(u8),
    EditSeconds(u8),
    ToggleEditRepeat,
    EditRepeatCount(u32),
    EditSound(String),
    StartTimer(u32),
    PauseTimer(u32),
    ResumeTimer(u32),
    ResetTimer(u32),
    DeleteTimer(u32),
    BrowseCustomSound,
    Tick,
}
