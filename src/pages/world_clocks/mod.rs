// SPDX-License-Identifier: MIT

mod coords;
mod model;
mod update;
mod view;

pub use model::*;

use chrono_tz::Tz;

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    AddClock(Tz),
    RemoveClock(u32),
    OpenAddSidebar,
    SelectClock(u32),
    DeselectClock,
    ToggleEditMode,
    StartDrag(usize),
    Reorder(usize, usize),
    FinishDrag,
    CancelDrag,
}

/// Extract the city name from a timezone identifier (last segment after `/`).
fn tz_city_name(tz: Tz) -> String {
    let name = tz.name();
    name.rsplit('/')
        .next()
        .unwrap_or(name)
        .replace('_', " ")
}

/// Extract the region/country name from a timezone identifier (first segment before `/`).
fn tz_region_name(tz: Tz) -> String {
    let name = tz.name();
    name.split('/')
        .next()
        .unwrap_or(name)
        .replace('_', " ")
}
