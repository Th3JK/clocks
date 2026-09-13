// SPDX-License-Identifier: MIT

mod model;
mod update;
mod view;

pub use model::*;

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Delete(u32),
    // Editing (context drawer)
    OpenSettings,
    StartEditEvent(u32),
    AddEvent,
    SaveEditEvent,
    CancelEdit,
    EditLabel(String),
    /// Calendar selection, as plain y/m/d so the jiff type stays in the view.
    EditDate(i32, u32, u32),
    ShowPrevMonth,
    ShowNextMonth,
    /// Open or close the calendar popup.
    ToggleCalendar,
    EditHour(u32),
    EditMinute(u32),
    ToggleYearly,
    ToggleReminder(Reminder),
    EditSound(String),
    BrowseCustomSound,
    // Edit mode (edit / delete)
    ToggleEditMode,
    /// Show one event full-page, as the world clocks and timers do.
    Focus(u32),
    Unfocus,
}
