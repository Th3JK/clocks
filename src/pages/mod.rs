// SPDX-License-Identifier: MIT

pub mod alarm;
pub mod pomodoro;
pub mod stopwatch;
pub mod timer;
pub mod world_clocks;

/// Navigation pages in the app
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    WorldClocks,
    Stopwatch,
    Alarm,
    Timer,
    Pomodoro,
}

/// Context drawer pages
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
    WorldClocksAdd,
    StopwatchHistory,
    AlarmEdit,
    TimerAdd,
    PomodoroSettings,
}
