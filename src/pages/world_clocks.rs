// SPDX-License-Identifier: MIT

use crate::fl;
use chrono::{Offset, TimeZone, Utc};
use chrono_tz::Tz;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;

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

        let city_name = tz_city_name(local_tz);

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

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    AddClock(Tz),
    RemoveClock(u32),
    OpenAddSidebar,
}

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

    /// Main view: page header + clock list
    pub fn view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = 12;
        let now_utc = Utc::now();

        let mut col = widget::column::with_capacity(self.clocks.len() + 2).spacing(spacing);

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("world-clocks-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .tooltip(fl!("tooltip-add"))
                    .on_press(Message::OpenAddSidebar),
            );
        col = col.push(header);

        for clock in &self.clocks {
            let time_in_tz = now_utc.with_timezone(&clock.timezone);
            let time_str = if use_12h {
                time_in_tz.format("%I:%M:%S %p").to_string()
            } else {
                time_in_tz.format("%H:%M:%S").to_string()
            };

            let offset_secs = clock
                .timezone
                .offset_from_utc_datetime(&now_utc.naive_utc())
                .fix()
                .local_minus_utc()
                - self
                    .local_timezone
                    .offset_from_utc_datetime(&now_utc.naive_utc())
                    .fix()
                    .local_minus_utc();
            let offset_hours = offset_secs as f64 / 3600.0;

            let offset_str = if clock.is_local {
                fl!("local")
            } else if offset_hours == 0.0 {
                fl!("same-time")
            } else if offset_hours > 0.0 {
                format!("+{:.0}h", offset_hours)
            } else {
                format!("{:.0}h", offset_hours)
            };

            let mut row = widget::row::with_capacity(3)
                .spacing(spacing)
                .align_y(Alignment::Center);

            row = row.push(
                widget::column::with_capacity(2)
                    .push(widget::text::body(&clock.city_name))
                    .push(widget::text::caption(offset_str))
                    .width(Length::Fill),
            );

            row = row.push(widget::text::title2(time_str));

            if !clock.is_local {
                let id = clock.id;
                row = row.push(
                    widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                        .tooltip(fl!("tooltip-remove"))
                        .on_press(Message::RemoveClock(id)),
                );
            }

            col = col.push(row);

            if clock.is_local {
                col = col.push(widget::divider::horizontal::default());
            }
        }

        col.into()
    }

    /// Sidebar view: search + timezone list for adding clocks
    pub fn sidebar_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(22).spacing(spacing);

        let search = widget::text_input(fl!("search-timezone"), &self.search_text)
            .on_input(Message::SearchChanged)
            .width(Length::Fill);
        col = col.push(search);

        if !self.filtered_timezones.is_empty() {
            for (name, tz) in &self.filtered_timezones {
                let tz_copy = *tz;
                let already_added = self.clocks.iter().any(|c| c.timezone == tz_copy);
                let label = if already_added {
                    fl!("timezone-added", name = name.clone())
                } else {
                    format!("{} ({})", name, tz.name())
                };
                let mut btn = widget::button::text(label);
                if !already_added {
                    btn = btn.on_press(Message::AddClock(tz_copy));
                }
                col = col.push(btn);
            }
        } else if self.search_text.is_empty() {
            col = col.push(widget::text::caption(fl!("type-to-search")));
        } else if self.search_text.len() < 2 {
            col = col.push(widget::text::caption(fl!("type-at-least-2")));
        } else {
            col = col.push(widget::text::caption(fl!("no-timezones-found")));
        }

        col.into()
    }
}

fn tz_city_name(tz: Tz) -> String {
    let name = tz.name();
    name.rsplit('/')
        .next()
        .unwrap_or(name)
        .replace('_', " ")
}
