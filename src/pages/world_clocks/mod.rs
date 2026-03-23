// SPDX-License-Identifier: MIT

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
}

fn tz_city_name(tz: Tz) -> String {
    let name = tz.name();
    name.rsplit('/')
        .next()
        .unwrap_or(name)
        .replace('_', " ")
}
