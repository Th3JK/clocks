// SPDX-License-Identifier: MIT
//
// Stopwatch view functions: main page view and history sidebar.

use super::model::*;
use super::Message;
use crate::components::{format_duration, format_duration_parts};
use crate::fl;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::Duration;

impl StopwatchState {
    pub fn view(&self) -> Element<'_, Message> {
        let spacing = 12;

        let mut col = widget::column::with_capacity(5)
            .spacing(spacing)
            .align_x(Alignment::Center)
            .width(Length::Fill);

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("stopwatch-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("addressbook-symbolic"))
                    .tooltip(fl!("tooltip-history"))
                    .on_press(Message::OpenHistory),
            );
        col = col.push(header);

        // Main time display — seconds bold, rest lighter
        let (prefix, seconds, suffix) = format_duration_parts(self.elapsed);
        let time_row = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text::title1(prefix).font(cosmic::font::default()))
            .push(widget::text::title1(seconds).font(cosmic::font::bold()))
            .push(widget::text::title1(suffix).font(cosmic::font::default()));
        col = col.push(
            widget::container(time_row)
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .padding(24),
        );

        // Control buttons
        let mut controls = widget::row::with_capacity(3)
            .spacing(spacing)
            .align_y(Alignment::Center);

        if self.is_running {
            controls = controls.push(widget::button::destructive(fl!("stop")).on_press(Message::Stop));
            controls = controls.push(widget::button::standard(fl!("lap")).on_press(Message::Lap));
        } else if self.elapsed > Duration::ZERO {
            controls = controls.push(widget::button::suggested(fl!("resume")).on_press(Message::Start));
            controls = controls.push(widget::button::standard(fl!("reset")).on_press(Message::Reset));
        } else {
            controls = controls.push(widget::button::suggested(fl!("start")).on_press(Message::Start));
        }

        col = col.push(
            widget::container(controls)
                .align_x(Alignment::Center)
                .width(Length::Fill),
        );

        // Laps display
        if !self.laps.is_empty() {
            col = col.push(widget::divider::horizontal::default());

            let mut laps_col = widget::column::with_capacity(self.laps.len()).spacing(4);
            let green = cosmic::iced::Color::from_rgb(0.2, 0.8, 0.2);
            let red = cosmic::iced::Color::from_rgb(0.9, 0.3, 0.3);

            for (i, lap) in self.laps.iter().enumerate().rev() {
                let lap_label = format!("{}: {}", fl!("lap-entry", id = lap.id.to_string()), format_duration(lap.lap_time));
                let mut row = widget::row::with_capacity(3)
                    .spacing(6)
                    .align_y(Alignment::Center);
                row = row.push(widget::text::body(lap_label));

                if i > 0 {
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
                    row = row.push(
                        widget::text::body(delta_str)
                            .class(cosmic::theme::Text::Color(delta_color)),
                    );
                }

                if lap.is_fastest {
                    row = row.push(
                        widget::text::caption(fl!("fastest"))
                            .class(cosmic::theme::Text::Color(green)),
                    );
                } else if lap.is_slowest {
                    row = row.push(
                        widget::text::caption(fl!("slowest"))
                            .class(cosmic::theme::Text::Color(red)),
                    );
                }

                laps_col = laps_col.push(row);
            }

            col = col.push(laps_col);
        }

        col.into()
    }

    /// History sidebar view
    pub fn history_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(self.history.len() + 2).spacing(spacing);

        if self.history.is_empty() {
            col = col.push(widget::text::body(fl!("no-history")));
            col = col.push(widget::text::caption(fl!("history-hint")));
        } else {
            col = col.push(
                widget::button::destructive(fl!("clear-all-history")).on_press(Message::ClearHistory),
            );

            for record in self.history.iter().rev() {
                let id = record.id;
                col = col.push(widget::divider::horizontal::default());

                // Editable label
                col = col.push(
                    widget::text_input(fl!("session-label"), &record.label)
                        .on_input(move |l| Message::EditHistoryLabel(id, l)),
                );

                col = col.push(widget::text::body(
                    fl!("total-time", time = format_duration(record.total_elapsed))
                ));

                if !record.laps.is_empty() {
                    col = col.push(widget::text::caption(
                        fl!("laps-recorded", count = record.laps.len().to_string())
                    ));
                }

                let actions = widget::row::with_capacity(2)
                    .spacing(8)
                    .push(
                        widget::button::suggested(fl!("resume"))
                            .on_press(Message::ResumeFromHistory(id)),
                    )
                    .push(
                        widget::button::destructive(fl!("delete")).on_press(Message::DeleteHistory(id)),
                    );
                col = col.push(actions);
            }
        }

        col.into()
    }
}
