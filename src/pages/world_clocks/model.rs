// SPDX-License-Identifier: MIT
//
// World clocks data types: clock entries and state.

use chrono_tz::Tz;

#[derive(Debug, Clone)]
pub struct ClockEntry {
    pub id: u32,
    pub timezone: Tz,
    pub city_name: String,
    pub is_local: bool,
}

pub struct WorldClocksState {
    pub local_timezone: Tz,
    pub clocks: Vec<ClockEntry>,
    pub next_id: u32,
    pub search_text: String,
    pub filtered_timezones: Vec<(String, Tz)>,
    pub selected_clock_id: Option<u32>,
}

impl Default for WorldClocksState {
    fn default() -> Self {
        let local_tz = iana_time_zone::get_timezone()
            .ok()
            .and_then(|tz_str| tz_str.parse::<Tz>().ok())
            .unwrap_or(chrono_tz::UTC);

        Self {
            local_timezone: local_tz,
            clocks: Vec::new(),
            next_id: 0,
            search_text: String::new(),
            filtered_timezones: Vec::new(),
            selected_clock_id: None,
        }
    }
}
