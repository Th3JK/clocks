// SPDX-License-Identifier: MIT
//
// Timer view functions: main page view and sidebar editing form.

use super::model::*;
use super::Message;
use crate::components::{format_duration_hms, sound_selector_view};
use crate::fl;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::Duration;

impl TimerState {
    /// Main view: page header + timer list
    pub fn view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(self.timers.len() + 3).spacing(spacing);

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("timer-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .tooltip(fl!("tooltip-add"))
                    .on_press(Message::StartNew),
            );
        col = col.push(header);

        if self.timers.is_empty() {
            col = col.push(
                widget::container(widget::text::body(fl!("no-timers")))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .padding(24),
            );
        }

        for timer in &self.timers {
            let remaining_str = format_duration_hms(timer.remaining);
            let id = timer.id;

            let mut row = widget::row::with_capacity(4)
                .spacing(spacing)
                .align_y(Alignment::Center);

            let mut info_col = widget::column::with_capacity(2);
            info_col = info_col.push(widget::text::body(&timer.label));
            if timer.repeat_enabled {
                let repeat_str = if timer.repeat_count == 0 {
                    fl!("repeat-progress-infinite", completed = timer.completed_count.to_string())
                } else {
                    fl!("repeat-progress", completed = timer.completed_count.to_string(), total = timer.repeat_count.to_string())
                };
                info_col = info_col.push(widget::text::caption(repeat_str));
            }

            row = row.push(info_col.width(Length::Fill));
            row = row.push(widget::text::title3(remaining_str));

            // Control buttons in a themed container
            let mut ctrl_row = widget::row::with_capacity(2)
                .spacing(4)
                .align_y(Alignment::Center);

            if timer.is_running {
                ctrl_row = ctrl_row.push(
                    widget::button::icon(widget::icon::from_name("media-playback-stop-symbolic"))
                        .tooltip(fl!("tooltip-pause"))
                        .on_press(Message::PauseTimer(id)),
                );
            } else if timer.remaining > Duration::ZERO {
                ctrl_row = ctrl_row.push(
                    widget::button::icon(widget::icon::from_name("media-playback-start-symbolic"))
                        .tooltip(fl!("tooltip-start"))
                        .on_press(if timer.remaining < timer.initial_duration {
                            Message::ResumeTimer(id)
                        } else {
                            Message::StartTimer(id)
                        }),
                );
            } else {
                ctrl_row = ctrl_row.push(
                    widget::icon::from_name("object-select-symbolic").size(24),
                );
            }

            ctrl_row = ctrl_row.push(
                widget::button::icon(widget::icon::from_name("edit-undo-symbolic"))
                    .tooltip(fl!("tooltip-reset"))
                    .on_press(Message::ResetTimer(id)),
            );

            row = row.push(ctrl_row);

            if !timer.is_running {
                row = row.push(
                    widget::button::icon(widget::icon::from_name("edit-symbolic"))
                        .tooltip(fl!("tooltip-edit"))
                        .on_press(Message::StartEditTimer(id)),
                );
            }
            row = row.push(
                widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                    .tooltip(fl!("tooltip-delete"))
                    .on_press(Message::DeleteTimer(id)),
            );

            col = col.push(row);
        }

        col.into()
    }

    /// Sidebar view: timer creation form
    pub fn sidebar_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(8).spacing(spacing);

        // Label
        col = col.push(widget::text::body(fl!("label")));
        col = col
            .push(
                widget::text_input(fl!("timer-label-placeholder"), &self.edit_label)
                    .id(widget::Id::new("timer-label-input"))
                    .on_input(Message::EditLabel),
            );

        // Duration spinners with wrap-around (HH:MM:SS colon format)
        col = col.push(widget::text::body(fl!("duration")));

        let h = self.edit_hours;
        let m = self.edit_minutes;
        let s = self.edit_seconds;

        let dur_row = widget::row::with_capacity(11)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                    .on_press(Message::EditHours(if h == 0 { 23 } else { h - 1 })),
            )
            .push(widget::text::title3(format!("{:02}", h)))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                    .on_press(Message::EditHours((h + 1) % 24)),
            )
            .push(widget::text::title3(":"))
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                    .on_press(Message::EditMinutes(if m == 0 { 59 } else { m - 1 })),
            )
            .push(widget::text::title3(format!("{:02}", m)))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                    .on_press(Message::EditMinutes((m + 1) % 60)),
            )
            .push(widget::text::title3(":"))
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                    .on_press(Message::EditSeconds(if s == 0 { 59 } else { s - 1 })),
            )
            .push(widget::text::title3(format!("{:02}", s)))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                    .on_press(Message::EditSeconds((s + 1) % 60)),
            );
        col = col.push(dur_row);

        // Repeat toggle
        col = col.push(widget::text::body(fl!("repeat")));
        let repeat_btn = if self.edit_repeat {
            widget::button::suggested(fl!("repeat-on")).on_press(Message::ToggleEditRepeat)
        } else {
            widget::button::standard(fl!("repeat-off")).on_press(Message::ToggleEditRepeat)
        };
        col = col.push(repeat_btn);

        // Repeat count (when repeat is on)
        if self.edit_repeat {
            let c = self.edit_repeat_count;
            let count_label = if c == 0 {
                "∞".to_string()
            } else {
                format!("{}", c)
            };

            let count_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("repeat-count")))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::EditRepeatCount(c.saturating_sub(1))),
                )
                .push(widget::text::title4(count_label))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::EditRepeatCount(c + 1)),
                );
            col = col.push(count_row);
            col = col.push(widget::text::caption(fl!("infinite-repeats")));
        }

        // Sound selection
        col = col.push(widget::divider::horizontal::default());
        col = col.push(sound_selector_view(
            fl!("sound"),
            &self.edit_sound,
            Message::EditSound,
            Message::BrowseCustomSound,
        ));

        // Actions
        col = col.push(widget::divider::horizontal::default());
        let save_label = if self.edit_id.is_some() {
            fl!("save")
        } else {
            fl!("add-timer")
        };
        let actions = widget::row::with_capacity(2)
            .spacing(8)
            .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEdit))
            .push(widget::button::suggested(save_label).on_press(Message::SaveTimer));
        col = col.push(actions);

        col.into()
    }
}
