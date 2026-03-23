// SPDX-License-Identifier: MIT

mod model;
mod update;
mod view;

pub use model::*;

#[derive(Debug, Clone)]
pub enum Message {
    Start,
    Stop,
    Reset,
    Lap,
    Tick,
    // History
    EditHistoryLabel(u32, String),
    DeleteHistory(u32),
    ResumeFromHistory(u32),
    ClearHistory,
    OpenHistory,
}
