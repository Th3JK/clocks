// SPDX-License-Identifier: MIT

mod model;
mod update;
mod view;

pub use model::*;

#[derive(Debug, Clone)]
pub enum Message {
    /// The given player tapped their panel — they finished their move.
    TapPlayer(Player),
    /// Toggle pause/resume of the running clock.
    PauseToggle,
    Reset,
    Tick,
    // Settings sidebar
    OpenSettings,
    EditBaseMinutes(u32),
    EditIncrementSecs(u32),
    ApplyPreset(u32, u32),
    ApplySettings,
}
