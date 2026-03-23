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
}

impl Default for WorldClocksState {
    fn default() -> Self {
        let local_tz = iana_time_zone::get_timezone()
            .ok()
            .and_then(|tz_str| tz_str.parse::<Tz>().ok())
            .unwrap_or(chrono_tz::UTC);

        let city_name = super::tz_city_name(local_tz);

        let local_clock = ClockEntry {
            id: 0,
            timezone: local_tz,
            city_name,
            is_local: true,
        };

        Self {
            local_timezone: local_tz,
            clocks: vec![local_clock],
            next_id: 1,
            search_text: String::new(),
            filtered_timezones: Vec::new(),
        }
    }
}
