// SPDX-License-Identifier: MIT
//
// World clocks view functions: main page view, detail view, and sidebar search.

use super::coords::approximate_coords;
use super::model::*;
use super::{Message, tz_city_name, tz_region_name};
use crate::components::reorder_list::ReorderList;
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

        if self.edit_mode {
            self.edit_mode_view(use_12h)
        } else {
            self.list_view(use_12h)
        }
    }

    /// List view (base/view mode): page header + clock list or empty state.
    /// Rows have no delete icon, no hover highlight, a right-facing chevron,
    /// and the entire row is a full-width clickable area.
    fn list_view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = 12;
        let now_utc = Utc::now();

        let mut col = widget::column::with_capacity(self.clocks.len() + 3).spacing(spacing);

        // Page header: title + edit button + add button
        let header = self.header_row();
        col = col.push(header);

        if self.clocks.is_empty() {
            col = col.push(self.empty_state());
        } else {
            // --- Clock list grouped in a list_column container ---
            let mut list_col = widget::list_column();

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

                // Chevron icon on far right
                let chevron = widget::icon::from_name("go-next-symbolic")
                    .size(16)
                    .icon();

                // Full-width clickable row: left + time pill + chevron
                let clickable = widget::row::with_capacity(3)
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(left)
                    .push(time_pill)
                    .push(chevron);

                // Use mouse_area for full-width click without hover highlight
                let row = widget::mouse_area(
                    widget::container(clickable)
                        .width(Length::Fill)
                        .padding([8, 0]),
                )
                .on_press(Message::SelectClock(id));

                list_col = list_col.add(row);
            }

            col = col.push(list_col);
        }

        col.into()
    }

    /// Edit mode view: drag handles, card rows, delete buttons.
    ///
    /// Follows the cosmic-settings panel applet list pattern:
    /// each row is an inline card built with standard cosmic primitives,
    /// wrapped in a `ReorderList` widget for drag-to-reorder via Wayland DnD.
    fn edit_mode_view(&self, use_12h: bool) -> Element<'_, Message> {
        let cosmic::cosmic_theme::Spacing {
            space_xxxs,
            space_xxs,
            space_xs,
            ..
        } = cosmic::theme::spacing();
        let now_utc = Utc::now();

        let mut col = widget::column::with_capacity(self.clocks.len() + 3).spacing(space_xxs);

        let header = self.header_row();
        col = col.push(header);

        if self.clocks.is_empty() {
            col = col.push(self.empty_state());
        } else {
            let dragging = self.dragging_index;

            let card_rows: Vec<Element<'_, Message>> = self
                .clocks
                .iter()
                .enumerate()
                .map(|(i, clock)| {
                    // Collapse the dragged item to an accent-colored drop indicator line
                    if dragging == Some(i) {
                        return widget::container(widget::Space::new().width(Length::Fill))
                            .height(Length::Fixed(4.0))
                            .width(Length::Fill)
                            .class(cosmic::theme::Container::Custom(Box::new(
                                |theme| {
                                    let accent = Color::from(theme.cosmic().accent_color());
                                    cosmic::iced_widget::container::Style {
                                        background: Some(
                                            cosmic::iced::Background::Color(accent),
                                        ),
                                        border: cosmic::iced::Border {
                                            radius: 2.0.into(),
                                            ..Default::default()
                                        },
                                        ..Default::default()
                                    }
                                },
                            )))
                            .into();
                    }

                    let time_in_tz = now_utc.with_timezone(&clock.timezone);
                    let time_str = if use_12h {
                        time_in_tz.format("%I:%M %p").to_string()
                    } else {
                        time_in_tz.format("%H:%M").to_string()
                    };

                    let offset_str = self.offset_description(clock.timezone);
                    let id = clock.id;

                    // Day/night time pill — same as view mode
                    let local_hour = time_in_tz.hour();
                    let pill_bg = if (6..20).contains(&local_hour) {
                        DAY_PILL_COLOR
                    } else {
                        NIGHT_PILL_COLOR
                    };
                    let time_pill = widget::container(
                        widget::text::title4(time_str).font(cosmic::font::bold()),
                    )
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

                    // Row content: drag handle | icon | text block | time pill | delete button
                    let content = widget::row::with_children(vec![
                        // Drag handle
                        widget::icon::from_name("grip-lines-symbolic")
                            .size(16)
                            .icon()
                            .class(cosmic::theme::Svg::Custom(std::rc::Rc::new(
                                |theme: &cosmic::Theme| cosmic::iced_widget::svg::Style {
                                    color: Some(theme.cosmic().palette.neutral_7.into()),
                                },
                            )))
                            .into(),
                        // Clock icon
                        widget::icon::from_name("preferences-system-time-symbolic")
                            .size(20)
                            .icon()
                            .into(),
                        // Text block: city name (primary) + offset (secondary)
                        widget::column::with_capacity(2)
                            .spacing(space_xxxs)
                            .width(Length::Fill)
                            .push(widget::text::body(&clock.city_name))
                            .push(widget::text::caption(offset_str))
                            .into(),
                        // Time pill
                        time_pill.into(),
                        // Delete button
                        widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                            .extra_small()
                            .tooltip(fl!("tooltip-remove"))
                            .on_press(Message::RemoveClock(id))
                            .into(),
                    ])
                    .spacing(space_xs)
                    .align_y(Alignment::Center);

                    // Card container: bg_component_color background, radius_s corners,
                    // accent border when this is the item being dragged elsewhere
                    widget::container(content)
                        .padding(8)
                        .width(Length::Fill)
                        .class(cosmic::theme::Container::Custom(Box::new(
                            move |theme| {
                                let mut style = cosmic::iced_widget::container::Catalog::style(
                                    theme,
                                    &cosmic::theme::Container::Primary,
                                );
                                style.border.radius = theme.cosmic().radius_s().into();
                                style.background = Some(
                                    Color::from(theme.cosmic().bg_component_color()).into(),
                                );
                                style
                            },
                        )))
                        .into()
                })
                .collect();

            let item_count = self.clocks.len();
            let cards = widget::column::with_children(card_rows).spacing(space_xxs);

            // Pre-clone clock data for the drag icon builder ('static closure)
            let clocks_snapshot: Vec<(String, String, Color)> = self
                .clocks
                .iter()
                .map(|clock| {
                    let time_in_tz = now_utc.with_timezone(&clock.timezone);
                    let time_str = if use_12h {
                        time_in_tz.format("%I:%M %p").to_string()
                    } else {
                        time_in_tz.format("%H:%M").to_string()
                    };
                    let local_hour = time_in_tz.hour();
                    let pill_bg = if (6..20).contains(&local_hour) {
                        DAY_PILL_COLOR
                    } else {
                        NIGHT_PILL_COLOR
                    };
                    (clock.city_name.clone(), time_str, pill_bg)
                })
                .collect();

            let reorder_list = ReorderList::new(cards, item_count, self.dragging_index)
                .on_start_drag(Message::StartDrag)
                .on_reorder(|from, to| Message::Reorder(from, to))
                .on_finish(Message::FinishDrag)
                .on_cancel(Message::CancelDrag)
                .drag_icon(move |index, offset| {
                    let (city, time_str, pill_bg) = clocks_snapshot
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| ("Clock".to_string(), String::new(), DAY_PILL_COLOR));

                    let time_pill = widget::container(
                        widget::text::title4(time_str).font(cosmic::font::bold()),
                    )
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

                    let content = widget::row::with_children(vec![
                        widget::icon::from_name("grip-lines-symbolic")
                            .size(16)
                            .icon()
                            .into(),
                        widget::icon::from_name("preferences-system-time-symbolic")
                            .size(20)
                            .icon()
                            .into(),
                        widget::text::body(city).width(Length::Fill).into(),
                        time_pill.into(),
                    ])
                    .spacing(space_xs)
                    .align_y(Alignment::Center);

                    // Card with accent border for the floating drag icon
                    let card: Element<'static, ()> = widget::container(content)
                        .padding(8)
                        .width(Length::Fill)
                        .class(cosmic::theme::Container::Custom(Box::new(
                            |theme| {
                                let accent = Color::from(theme.cosmic().accent_color());
                                let mut style = cosmic::iced_widget::container::Catalog::style(
                                    theme,
                                    &cosmic::theme::Container::Primary,
                                );
                                style.border.radius = theme.cosmic().radius_s().into();
                                style.border.color = accent;
                                style.border.width = 2.0;
                                style.background = Some(
                                    Color::from(theme.cosmic().bg_component_color()).into(),
                                );
                                style
                            },
                        )))
                        .into();

                    (card, cosmic::iced_core::widget::tree::State::None, offset)
                });

            col = col.push(reorder_list);
        }

        col.into()
    }

    /// Shared header row: title + edit button (if clocks exist) + add button.
    fn header_row(&self) -> Element<'_, Message> {
        let mut header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("world-clocks-title")).width(Length::Fill));

        // Only show edit button when there are clocks to edit
        if !self.clocks.is_empty() {
            let (edit_icon, edit_tooltip) = if self.edit_mode {
                ("object-select-symbolic", fl!("tooltip-done-editing"))
            } else {
                ("edit-symbolic", fl!("tooltip-edit-mode"))
            };
            header = header.push(
                widget::button::icon(widget::icon::from_name(edit_icon))
                    .tooltip(edit_tooltip)
                    .on_press(Message::ToggleEditMode),
            );
        }

        header = header.push(
            widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                .tooltip(fl!("tooltip-add"))
                .on_press(Message::OpenAddSidebar),
        );

        header.into()
    }

    /// Shared empty state: centered globe icon + CTA button.
    fn empty_state(&self) -> Element<'_, Message> {
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

        widget::container(empty_state)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
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
