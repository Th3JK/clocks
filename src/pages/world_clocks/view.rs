// SPDX-License-Identifier: MIT
//
// World clocks view functions: main page view, detail view, and sidebar search.

use super::coords::approximate_coords;
use super::model::*;
use super::{Message, tz_city_name, tz_region_name};
use crate::fl;
use chrono::{Datelike, NaiveDate, Offset, TimeZone, Timelike, Utc};
use cosmic::iced::{Alignment, Color, Length};
use cosmic::prelude::*;
use cosmic::widget;
use sunrise::{Coordinates, SolarDay, SolarEvent};

/// Pill background for daytime hours (06:00–19:59): warm amber/orange.
const DAY_PILL_COLOR: Color = Color {
    r: 0.545,
    g: 0.290,
    b: 0.000,
    a: 0.85,
};

/// Pill background for nighttime hours (20:00–05:59): cool deep blue.
const NIGHT_PILL_COLOR: Color = Color {
    r: 0.000,
    g: 0.169,
    b: 0.361,
    a: 0.85,
};

impl WorldClocksState {
    /// Compute the offset in whole hours between the given timezone and the local timezone.
    fn offset_hours_from_local(&self, tz: chrono_tz::Tz) -> f64 {
        let now_utc = Utc::now();
        let offset_secs = tz
            .offset_from_utc_datetime(&now_utc.naive_utc())
            .fix()
            .local_minus_utc()
            - self
                .local_timezone
                .offset_from_utc_datetime(&now_utc.naive_utc())
                .fix()
                .local_minus_utc();
        offset_secs as f64 / 3600.0
    }

    /// Build a human-readable offset description for a timezone.
    fn offset_description(&self, tz: chrono_tz::Tz) -> String {
        let offset_hours = self.offset_hours_from_local(tz);
        if tz == self.local_timezone {
            fl!("world-clocks-current-timezone")
        } else if offset_hours == 0.0 {
            fl!("world-clocks-same-time")
        } else if offset_hours > 0.0 {
            let h = offset_hours.round() as i64;
            fl!("world-clocks-hours-ahead", hours = h.to_string())
        } else {
            let h = (-offset_hours).round() as i64;
            fl!("world-clocks-hours-behind", hours = h.to_string())
        }
    }

    /// Main view: page header + clock list, empty state, or detail view.
    pub fn view(&self, use_12h: bool) -> Element<'_, Message> {
        // If a clock is selected and still exists, show the detail view
        if let Some(id) = self.selected_clock_id
            && let Some(clock) = self.clocks.iter().find(|c| c.id == id)
        {
            return self.detail_view(clock, use_12h);
        }

        self.list_view(use_12h)
    }

    /// List view: page header + clock list or empty state.
    fn list_view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = 12;
        let now_utc = Utc::now();

        let mut col = widget::column::with_capacity(self.clocks.len() + 3).spacing(spacing);

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

        if self.clocks.is_empty() {
            // --- Empty state: centered globe icon + CTA button ---
            let icon = widget::icon::from_name("preferences-system-time-symbolic")
                .size(128)
                .icon()
                .class(cosmic::theme::Svg::Custom(std::rc::Rc::new(
                    |theme: &cosmic::Theme| cosmic::iced_widget::svg::Style {
                        color: Some(theme.cosmic().palette.neutral_5.into()),
                    },
                )));

            let empty_state = widget::column::with_capacity(2)
                .spacing(16)
                .align_x(Alignment::Center)
                .push(icon)
                .push(
                    widget::button::suggested(fl!("world-clocks-add-button"))
                        .on_press(Message::OpenAddSidebar),
                );

            col = col.push(
                widget::container(empty_state)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(Length::Fill)
                    .height(Length::Fill),
            );
        } else {
            // --- Clock list grouped in a list_column container ---
            let mut list = widget::list_column();

            for clock in &self.clocks {
                let time_in_tz = now_utc.with_timezone(&clock.timezone);
                let time_str = if use_12h {
                    time_in_tz.format("%I:%M %p").to_string()
                } else {
                    time_in_tz.format("%H:%M").to_string()
                };

                let offset_str = self.offset_description(clock.timezone);
                let id = clock.id;

                // Determine day/night pill color based on local hour
                let local_hour = time_in_tz.hour();
                let pill_bg = if (6..20).contains(&local_hour) {
                    DAY_PILL_COLOR
                } else {
                    NIGHT_PILL_COLOR
                };

                // Left side: city name + offset description
                let left = widget::column::with_capacity(2)
                    .push(widget::text::body(&clock.city_name))
                    .push(widget::text::caption(offset_str))
                    .width(Length::Fill);

                // Right side: time pill with day/night background
                let time_pill =
                    widget::container(widget::text::title4(time_str).font(cosmic::font::bold()))
                        .class(cosmic::theme::Container::custom(move |_theme| {
                            cosmic::iced_widget::container::Style {
                                background: Some(cosmic::iced::Background::Color(pill_bg)),
                                border: cosmic::iced::Border {
                                    radius: 8.0.into(),
                                    ..Default::default()
                                },
                                text_color: Some(Color::WHITE),
                                ..Default::default()
                            }
                        }))
                        .padding([6, 14]);

                // Clickable area: left + time pill
                let clickable = widget::row::with_capacity(2)
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(left)
                    .push(time_pill);

                let row_btn = widget::button::custom(clickable)
                    .width(Length::Fill)
                    .on_press(Message::SelectClock(id))
                    .class(cosmic::theme::Button::ListItem);

                // Delete button sits outside the ListItem button
                let delete_btn =
                    widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                        .tooltip(fl!("tooltip-remove"))
                        .on_press(Message::RemoveClock(id));

                let row = widget::row::with_capacity(2)
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(row_btn)
                    .push(delete_btn);

                list = list.add(row);
            }

            col = col.push(list);
        }

        col.into()
    }

    /// Detail view for a single selected clock.
    fn detail_view<'a>(&'a self, clock: &'a ClockEntry, use_12h: bool) -> Element<'a, Message> {
        let now_utc = Utc::now();
        let time_in_tz = now_utc.with_timezone(&clock.timezone);

        // ── Header: back button + city / country centered ──
        let back_btn = widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
            .on_press(Message::DeselectClock);

        let country = tz_region_name(clock.timezone);

        let header_text = widget::column::with_capacity(2)
            .align_x(Alignment::Center)
            .push(widget::text::title3(&clock.city_name))
            .push(widget::text::caption(country))
            .width(Length::Fill);

        // Spacer to balance the back button so the text is centered
        let header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(back_btn)
            .push(header_text)
            .push(widget::Space::new().width(40.0));

        // ── Center: large time display ──
        let time_str = if use_12h {
            time_in_tz.format("%I:%M:%S %p").to_string()
        } else {
            time_in_tz.format("%H:%M:%S").to_string()
        };

        let big_time =
            widget::container(widget::text(time_str).size(72.0).font(cosmic::font::bold()))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .height(Length::Fill);

        // ── Bottom: sunrise / sunset ──
        let date = time_in_tz.date_naive();
        let (sunrise_str, sunset_str) = match approximate_coords(clock.timezone.name()) {
            Some((lat, lon)) => match Coordinates::new(lat, lon) {
                Some(coord) => {
                    let naive_date = NaiveDate::from_ymd_opt(date.year(), date.month(), date.day());
                    match naive_date {
                        Some(nd) => {
                            let solar = SolarDay::new(coord, nd);
                            let sr = solar.event_time(SolarEvent::Sunrise);
                            let ss = solar.event_time(SolarEvent::Sunset);
                            (
                                Self::format_sun_dt(sr, clock.timezone, use_12h),
                                Self::format_sun_dt(ss, clock.timezone, use_12h),
                            )
                        }
                        None => (
                            fl!("world-clocks-no-sun-data"),
                            fl!("world-clocks-no-sun-data"),
                        ),
                    }
                }
                None => (
                    fl!("world-clocks-no-sun-data"),
                    fl!("world-clocks-no-sun-data"),
                ),
            },
            None => (
                fl!("world-clocks-no-sun-data"),
                fl!("world-clocks-no-sun-data"),
            ),
        };

        let label_width = Length::Fixed(80.0);

        let sunrise_row = widget::row::with_capacity(2)
            .spacing(16)
            .push(widget::text::caption(fl!("world-clocks-detail-sunrise")).width(label_width))
            .push(widget::text::body(sunrise_str).font(cosmic::font::bold()));

        let sunset_row = widget::row::with_capacity(2)
            .spacing(16)
            .push(widget::text::caption(fl!("world-clocks-detail-sunset")).width(label_width))
            .push(widget::text::body(sunset_str).font(cosmic::font::bold()));

        let sun_info = widget::container(
            widget::column::with_capacity(2)
                .spacing(4)
                .align_x(Alignment::Center)
                .push(sunrise_row)
                .push(sunset_row),
        )
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .padding([0, 0, 16, 0]);

        // ── Full layout ──
        widget::column::with_capacity(3)
            .push(header)
            .push(big_time)
            .push(sun_info)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Format a UTC DateTime as a local time string for sunrise/sunset display.
    fn format_sun_dt(dt: chrono::DateTime<Utc>, tz: chrono_tz::Tz, use_12h: bool) -> String {
        let local = dt.with_timezone(&tz);
        if use_12h {
            local.format("%I:%M %p").to_string()
        } else {
            local.format("%H:%M").to_string()
        }
    }

    /// Sidebar view: search + timezone list for adding clocks
    pub fn sidebar_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(22).spacing(spacing);

        let search = widget::text_input(fl!("search-timezone"), &self.search_text)
            .id(widget::Id::new("world-clocks-search-input"))
            .on_input(Message::SearchChanged)
            .width(Length::Fill);
        col = col.push(search);

        if !self.filtered_timezones.is_empty() {
            for (_name, tz) in &self.filtered_timezones {
                let tz_copy = *tz;
                let already_added = self.clocks.iter().any(|c| c.timezone == tz_copy);

                let city = tz_city_name(tz_copy);
                let region = tz_region_name(tz_copy);
                let tz_identifier = tz.name().to_string();
                let offset_desc = self.offset_description(tz_copy);

                if already_added {
                    // Show as disabled row (no press action)
                    let label_text = fl!("timezone-added", name = city);
                    col = col.push(
                        widget::container(widget::text::caption(label_text)).padding([8, 12]),
                    );
                } else {
                    // First line: city (normal) + region (bold)
                    let first_line = widget::row::with_capacity(2)
                        .spacing(6)
                        .push(widget::text::body(city))
                        .push(widget::text::body(region).font(cosmic::font::bold()));

                    // Second line: timezone identifier + offset description
                    let second_line_text = format!("{} \u{2022} {}", tz_identifier, offset_desc);
                    let second_line = widget::text::caption(second_line_text);

                    let row_content = widget::column::with_capacity(2)
                        .push(first_line)
                        .push(second_line)
                        .width(Length::Fill);

                    let btn = widget::button::custom(row_content)
                        .width(Length::Fill)
                        .on_press(Message::AddClock(tz_copy));
                    col = col.push(btn);
                }
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
