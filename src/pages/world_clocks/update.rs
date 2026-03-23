// SPDX-License-Identifier: MIT
//
// World clocks update logic: search, add, and remove clocks.

use super::model::*;
use super::{tz_city_name, Message};

impl WorldClocksState {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SearchChanged(text) => {
                self.search_text = text.clone();
                if text.len() >= 2 {
                    let lower = text.to_lowercase();
                    self.filtered_timezones = chrono_tz::TZ_VARIANTS
                        .iter()
                        .filter(|tz| {
                            let name = tz.name().to_lowercase();
                            name.contains(&lower)
                                || tz_city_name(**tz).to_lowercase().contains(&lower)
                        })
                        .take(20)
                        .map(|tz| (tz_city_name(*tz), *tz))
                        .collect();
                } else {
                    self.filtered_timezones.clear();
                }
            }
            Message::AddClock(tz) => {
                if !self.clocks.iter().any(|c| c.timezone == tz) {
                    self.clocks.push(ClockEntry {
                        id: self.next_id,
                        timezone: tz,
                        city_name: tz_city_name(tz),
                        is_local: false,
                    });
                    self.next_id += 1;
                }
                self.search_text.clear();
                self.filtered_timezones.clear();
            }
            Message::RemoveClock(id) => {
                self.clocks.retain(|c| c.id != id || c.is_local);
            }
            Message::OpenAddSidebar => {
                // Handled in app.rs
            }
        }
    }
}
