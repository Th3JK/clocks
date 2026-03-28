// SPDX-License-Identifier: MIT

mod model;
mod update;
mod view;

pub use model::*;

#[derive(Debug, Clone)]
pub enum Message {
    ToggleAlarm(u32),
    DeleteAlarm(u32),
    StartNewAlarm,
    StartEditAlarm(u32),
    CancelEdit,
    SaveAlarm,
    IncrementHour,
    DecrementHour,
    IncrementMinute,
    DecrementMinute,
    EditLabel(String),
    EditRepeatOnce,
    EditRepeatEveryDay,
    ToggleDay(DayOfWeek),
    EditSound(String),
    EditSnoozeMinutes(u8),
    EditRingMinutes(u8),
    BrowseCustomSound,
    ToggleAmPm(bool),
    SnoozeAlarm(u32),
    DismissAlarm(u32),
    ToggleEditMode,
    StartDrag(usize),
    Reorder(usize, usize),
    FinishDrag,
    CancelDrag,
}
