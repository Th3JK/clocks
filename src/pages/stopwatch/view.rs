// SPDX-License-Identifier: MIT
//
// Stopwatch view functions: main page view and history sidebar.

use super::model::*;
use super::Message;
use crate::components::{format_duration, format_duration_parts};
use crate::fl;
use cosmic::iced::font::Weight;
use cosmic::iced::{Alignment, Color, Length};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::Duration;

/// Font weight 300 (Light) for the hero time display.
fn light_font() -> cosmic::iced::Font {
    cosmic::iced::Font {
        weight: Weight::Light,
        ..cosmic::font::default()
    }
}

impl StopwatchState {
    pub fn view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let has_laps = !self.laps.is_empty();
        let has_run = self.elapsed > Duration::ZERO || has_laps;

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("stopwatch-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("addressbook-symbolic"))
                    .tooltip(fl!("tooltip-history"))
                    .on_press(Message::OpenHistory),
            );

        // Main time display — seconds bold, rest weight 300 (Light), large hero size
        let (prefix, seconds, suffix) = format_duration_parts(self.elapsed);
        let time_row = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text(prefix).size(72.0).font(light_font()))
            .push(widget::text(seconds).size(72.0).font(cosmic::font::bold()))
            .push(widget::text(suffix).size(72.0).font(light_font()));
        let time_display = widget::container(time_row)
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .padding(24);

        // --- Primary action button: Play / Pause (large icon in colored circle) ---
        let (primary_icon, primary_tooltip, primary_msg, use_accent) = if self.is_running {
            (
                "media-playback-pause-symbolic",
                fl!("tooltip-pause"),
                Message::Stop,
                false,
            )
        } else if has_run {
            (
                "media-playback-start-symbolic",
                fl!("tooltip-resume"),
                Message::Start,
                true,
            )
        } else {
            (
                "media-playback-start-symbolic",
                fl!("tooltip-start"),
                Message::Start,
                true,
            )
        };

        let primary_btn_inner =
            widget::icon::from_name(primary_icon).size(32).icon();

        let primary_btn = widget::tooltip(
            widget::button::custom(
                widget::container(primary_btn_inner)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(64)
                    .height(64),
            )
            .class(if use_accent {
                cosmic::theme::Button::Suggested
            } else {
                cosmic::theme::Button::Standard
            })
            .on_press(primary_msg),
            widget::text::body(primary_tooltip),
            widget::tooltip::Position::Top,
        );

        // --- Secondary action button: Lap (running) or Reset (paused with time) ---
        // 2/3 size of primary (64 * 2/3 ≈ 42), with background
        let secondary_btn: Option<Element<'_, Message>> = if self.is_running {
            let icon = widget::icon::from_name("pin-symbolic").size(20).icon();
            Some(
                widget::tooltip(
                    widget::button::custom(
                        widget::container(icon)
                            .align_x(Alignment::Center)
                            .align_y(Alignment::Center)
                            .width(42)
                            .height(42),
                    )
                    .class(cosmic::theme::Button::Standard)
                    .on_press(Message::Lap),
                    widget::text::body(fl!("tooltip-lap")),
                    widget::tooltip::Position::Top,
                )
                .into(),
            )
        } else if has_run {
            let icon = widget::icon::from_name("edit-undo-symbolic").size(20).icon();
            Some(
                widget::tooltip(
                    widget::button::custom(
                        widget::container(icon)
                            .align_x(Alignment::Center)
                            .align_y(Alignment::Center)
                            .width(42)
                            .height(42),
                    )
                    .class(cosmic::theme::Button::Standard)
                    .on_press(Message::Reset),
                    widget::text::body(fl!("tooltip-reset")),
                    widget::tooltip::Position::Top,
                )
                .into(),
            )
        } else {
            None
        };

        // Fixed layout: primary always centered, secondary to its right.
        // Both side slots use identical disabled buttons (fully transparent)
        // so the primary never shifts — button padding is the same on both sides.
        let invisible_btn = || -> Element<'_, Message> {
            widget::button::custom(
                widget::container(widget::Space::new())
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(42)
                    .height(42),
            )
            .class(cosmic::theme::Button::Custom {
                active: Box::new(|_, _| widget::button::Style::default()),
                disabled: Box::new(|_| widget::button::Style::default()),
                hovered: Box::new(|_, _| widget::button::Style::default()),
                pressed: Box::new(|_, _| widget::button::Style::default()),
            })
            .into()
        };

        let secondary_slot: Element<'_, Message> =
            secondary_btn.unwrap_or_else(invisible_btn);
        let left_spacer: Element<'_, Message> = invisible_btn();

        let controls = widget::row::with_capacity(3)
            .spacing(spacing.space_s)
            .align_y(Alignment::Center)
            .push(left_spacer)
            .push(primary_btn)
            .push(secondary_slot);

        let controls_container = widget::container(controls)
            .align_x(Alignment::Center)
            .width(Length::Fill);

        // --- Build layout based on whether laps exist ---
        // Fixed width for time + laps block so the lap list matches the timer width.
        // The time display at 72px renders ~10 characters ("00:00:00.0") ≈ 500px.
        let content_width = Length::Fixed(500.0);

        if has_laps {
            // Laps exist: time at top, controls, then lap list
            let mut col = widget::column::with_capacity(4)
                .spacing(spacing.space_s)
                .align_x(Alignment::Center)
                .width(Length::Fill);

            col = col.push(header);
            col = col.push(time_display);
            col = col.push(controls_container);
            col = col.push(
                widget::container(self.laps_view())
                    .width(content_width)
                    .align_x(Alignment::Center)
                    .apply(|c| widget::container(c).align_x(Alignment::Center).width(Length::Fill)),
            );

            col.into()
        } else {
            // No laps: time + controls vertically centered on page
            let centered_content = widget::column::with_capacity(2)
                .spacing(spacing.space_m)
                .align_x(Alignment::Center)
                .push(time_display)
                .push(controls_container);

            let centered = widget::container(centered_content)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .height(Length::Fill);

            widget::column::with_capacity(2)
                .push(header)
                .push(centered)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }

    /// Lap records displayed using list_column with three columns:
    /// lap time | difference | lap label
    fn laps_view(&self) -> Element<'_, Message> {
        let green = Color::from_rgb(0.2, 0.8, 0.2);
        let red = Color::from_rgb(0.9, 0.3, 0.3);

        let mut list = widget::list_column();

        for (i, lap) in self.laps.iter().enumerate().rev() {
            let lap_time_str = format_duration(lap.lap_time);

            // Delta + fastest/slowest column
            let delta_content: Element<'_, Message> = if i > 0 {
                let (delta_str, delta_color) = if lap.delta < 0 {
                    (
                        format!(
                            "-{}",
                            format_duration(Duration::from_millis((-lap.delta) as u64))
                        ),
                        green,
                    )
                } else {
                    (
                        format!(
                            "+{}",
                            format_duration(Duration::from_millis(lap.delta as u64))
                        ),
                        red,
                    )
                };
                let mut delta_row = widget::row::with_capacity(2)
                    .spacing(6)
                    .align_y(Alignment::Center)
                    .push(
                        widget::text::body(delta_str)
                            .class(cosmic::theme::Text::Color(delta_color)),
                    );
                if lap.is_fastest {
                    delta_row = delta_row.push(
                        widget::text::caption(fl!("fastest"))
                            .class(cosmic::theme::Text::Color(green)),
                    );
                } else if lap.is_slowest {
                    delta_row = delta_row.push(
                        widget::text::caption(fl!("slowest"))
                            .class(cosmic::theme::Text::Color(red)),
                    );
                }
                delta_row.into()
            } else {
                // First lap: no difference to show
                widget::Space::new().into()
            };

            let lap_label = fl!("lap-entry", id = lap.id.to_string());

            let row = widget::row::with_capacity(3)
                .spacing(16)
                .align_y(Alignment::Center)
                .push(widget::text::body(lap_time_str).width(Length::FillPortion(3)))
                .push(
                    widget::container(delta_content)
                        .width(Length::FillPortion(3))
                        .align_x(Alignment::Center),
                )
                .push(
                    widget::container(widget::text::body(lap_label))
                        .width(Length::FillPortion(3))
                        .align_x(Alignment::End),
                );

            list = list.add(row);
        }

        list.into()
    }

    /// History sidebar view
    pub fn history_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col =
            widget::column::with_capacity(self.history.len() + 2).spacing(spacing.space_s);

        if self.history.is_empty() {
            col = col.push(widget::text::body(fl!("no-history")));
            col = col.push(widget::text::caption(fl!("history-hint")));
        } else {
            col = col.push(
                widget::button::destructive(fl!("clear-all-history"))
                    .on_press(Message::ClearHistory),
            );

            for record in self.history.iter().rev() {
                let id = record.id;

                // Card content
                let mut card_col =
                    widget::column::with_capacity(4).spacing(spacing.space_xxs);

                // Editable label
                card_col = card_col.push(
                    widget::text_input(fl!("session-label"), &record.label)
                        .on_input(move |l| Message::EditHistoryLabel(id, l)),
                );

                card_col = card_col.push(widget::text::body(fl!(
                    "total-time",
                    time = format_duration(record.total_elapsed)
                )));

                if !record.laps.is_empty() {
                    card_col = card_col.push(widget::text::caption(fl!(
                        "laps-recorded",
                        count = record.laps.len().to_string()
                    )));
                }

                let actions = widget::row::with_capacity(2)
                    .spacing(spacing.space_xs)
                    .push(
                        widget::button::suggested(fl!("resume"))
                            .on_press(Message::ResumeFromHistory(id)),
                    )
                    .push(
                        widget::button::destructive(fl!("delete"))
                            .on_press(Message::DeleteHistory(id)),
                    );
                card_col = card_col.push(actions);

                // Wrap in a themed card container
                let card = widget::container(card_col)
                    .padding(spacing.space_s)
                    .width(Length::Fill)
                    .class(cosmic::theme::Container::Custom(Box::new(
                        |theme| {
                            let mut style =
                                cosmic::iced_widget::container::Catalog::style(
                                    theme,
                                    &cosmic::theme::Container::Primary,
                                );
                            style.border.radius = theme.cosmic().radius_s().into();
                            style.background = Some(
                                Color::from(theme.cosmic().bg_component_color()).into(),
                            );
                            style
                        },
                    )));

                col = col.push(card);
            }
        }

        col.into()
    }
}
