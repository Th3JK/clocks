// SPDX-License-Identifier: MIT

pub mod alarm;
pub mod chess;
pub mod countdown;
pub mod pomodoro;
pub mod stopwatch;
pub mod timer;
pub mod workout;
pub mod world_clocks;

/// Navigation pages in the app
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    WorldClocks,
    Stopwatch,
    Alarm,
    Timer,
    Pomodoro,
    Chess,
    Workout,
    Countdown,
}

impl Page {
    /// Every page, in the order a fresh install shows them.
    pub const ALL: [Page; 8] = [
        Page::WorldClocks,
        Page::Stopwatch,
        Page::Alarm,
        Page::Timer,
        Page::Pomodoro,
        Page::Chess,
        Page::Workout,
        Page::Countdown,
    ];

    /// Stable identifier for the config.
    ///
    /// Deliberately a string rather than the enum's position: the sidebar order
    /// is user-defined and persisted, and keying it by index would scramble
    /// every saved layout the moment a page is inserted or reordered here.
    pub fn key(self) -> &'static str {
        match self {
            Page::WorldClocks => "world_clocks",
            Page::Stopwatch => "stopwatch",
            Page::Alarm => "alarm",
            Page::Timer => "timer",
            Page::Pomodoro => "pomodoro",
            Page::Chess => "chess",
            Page::Workout => "workout",
            Page::Countdown => "countdown",
        }
    }

    /// Resolve a stored key. Unknown keys are dropped rather than failing the
    /// load, so a config from a newer build still opens.
    pub fn from_key(key: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.key() == key)
    }
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
    ChessSettings,
    WorkoutEdit,
    CountdownEdit,
}
