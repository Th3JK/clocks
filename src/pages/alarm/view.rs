// SPDX-License-Identifier: MIT
//
// Alarm view functions: main page view and sidebar editing form.

use super::model::*;
use super::update::hour24_to_12;
use super::Message;
use crate::components::sound_selector_view;
use crate::fl;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;

impl AlarmState {
    /// Main view: page header + alarm list
    pub fn view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(self.alarms.len() + 3)
            .spacing(spacing);

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("alarms-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .tooltip(fl!("tooltip-add"))
                    .on_press(Message::StartNewAlarm),
            );
        col = col.push(header);

        // Ringing alarms are now shown via the floating dialog (Application::dialog())

        if self.alarms.is_empty() {
            col = col.push(
                widget::container(widget::text::body(fl!("no-alarms")))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .padding(24),
            );
        }

        for alarm in &self.alarms {
            let time_str = if use_12h {
                let (h12, is_pm) = hour24_to_12(alarm.hour);
                let period = if is_pm { fl!("pm") } else { fl!("am") };
                format!("{:02}:{:02} {}", h12, alarm.minute, period)
            } else {
                format!("{:02}:{:02}", alarm.hour, alarm.minute)
            };

            let id = alarm.id;
            let row = widget::row::with_capacity(5)
                .spacing(spacing)
                .align_y(Alignment::Center)
                .push(
                    widget::column::with_capacity(3)
                        .push(widget::text::body(&alarm.label))
                        .push(widget::text::title3(time_str))
                        .push(widget::text::caption(format!(
                            "{}",
                            alarm.repeat_mode
                        )))
                        .width(Length::Fill),
                )
                .push(
                    widget::button::icon(widget::icon::from_name("edit-symbolic"))
                        .tooltip(fl!("tooltip-edit"))
                        .on_press(Message::StartEditAlarm(id)),
                )
                .push(
                    widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                        .tooltip(fl!("tooltip-delete"))
                        .on_press(Message::DeleteAlarm(id)),
                )
                .push(
                    widget::toggler(alarm.is_enabled)
                        .on_toggle(move |_| Message::ToggleAlarm(id)),
                );

            col = col.push(row);
        }

        col.into()
    }

    /// Sidebar view: alarm editing form
    pub fn sidebar_view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(10).spacing(spacing);

        if let Some(edit) = &self.editing {
            // Label
            col = col.push(widget::text::body(fl!("label")));
            col = col
                .push(
                    widget::text_input(fl!("alarm-label-placeholder"), &edit.label)
                        .id(widget::Id::new("alarm-label-input"))
                        .on_input(Message::EditLabel),
                );

            // Time spinners with wrap-around
            let hour_str = format!("{:02}", edit.hour);
            let minute_str = format!("{:02}", edit.minute);

            col = col.push(widget::text::body(fl!("time")));
            let time_row = widget::row::with_capacity(8)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::DecrementHour),
                )
                .push(widget::text::title3(hour_str))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::IncrementHour),
                )
                .push(widget::text::title3(":"))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::DecrementMinute),
                )
                .push(widget::text::title3(minute_str))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::IncrementMinute),
                );
            col = col.push(time_row);

            // AM/PM selector (only in 12h mode)
            if use_12h {
                let am_btn = if edit.is_pm {
                    widget::button::standard(fl!("am")).on_press(Message::ToggleAmPm(false))
                } else {
                    widget::button::suggested(fl!("am")).on_press(Message::ToggleAmPm(false))
                };
                let pm_btn = if edit.is_pm {
                    widget::button::suggested(fl!("pm")).on_press(Message::ToggleAmPm(true))
                } else {
                    widget::button::standard(fl!("pm")).on_press(Message::ToggleAmPm(true))
                };
                let ampm_row = widget::row::with_capacity(2)
                    .spacing(8)
                    .push(am_btn)
                    .push(pm_btn);
                col = col.push(ampm_row);
            }

            // Repeat mode with highlighted selection
            col = col.push(widget::text::body(fl!("repeat")));

            let is_once = matches!(edit.repeat_mode, RepeatMode::Once);
            let is_everyday = matches!(edit.repeat_mode, RepeatMode::EveryDay);

            let once_btn = if is_once {
                widget::button::suggested(fl!("once")).on_press(Message::EditRepeatOnce)
            } else {
                widget::button::standard(fl!("once")).on_press(Message::EditRepeatOnce)
            };
            let everyday_btn = if is_everyday {
                widget::button::suggested(fl!("every-day")).on_press(Message::EditRepeatEveryDay)
            } else {
                widget::button::standard(fl!("every-day")).on_press(Message::EditRepeatEveryDay)
            };

            let repeat_row = widget::row::with_capacity(2)
                .spacing(8)
                .push(once_btn)
                .push(everyday_btn);
            col = col.push(repeat_row);

            // Day toggles
            col = col.push(widget::text::caption(fl!("select-specific-days")));
            let selected_days = match &edit.repeat_mode {
                RepeatMode::Custom(days) => days.clone(),
                _ => Vec::new(),
            };
            let mut days_row = widget::row::with_capacity(7).spacing(4);
            for day in DayOfWeek::all() {
                let is_selected = selected_days.contains(day);
                if is_selected {
                    days_row = days_row.push(
                        widget::button::suggested(day.display_name())
                            .on_press(Message::ToggleDay(*day)),
                    );
                } else {
                    days_row = days_row.push(
                        widget::button::standard(day.display_name())
                            .on_press(Message::ToggleDay(*day)),
                    );
                }
            }
            col = col.push(days_row);

            // Snooze duration
            col = col.push(widget::divider::horizontal::default());
            col = col.push(widget::text::body(fl!("snooze-duration")));
            let snz = edit.snooze_minutes;
            let snooze_row = widget::row::with_capacity(3)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::EditSnoozeMinutes(snz.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = snz.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::EditSnoozeMinutes(snz + 1)),
                );
            col = col.push(snooze_row);

            // Ring duration
            col = col.push(widget::text::body(fl!("ring-duration")));
            let ring = edit.ring_minutes;
            let ring_row = widget::row::with_capacity(3)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::EditRingMinutes(ring.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = ring.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::EditRingMinutes(ring + 1)),
                );
            col = col.push(ring_row);

            // Sound selection
            col = col.push(widget::divider::horizontal::default());
            col = col.push(sound_selector_view(
                fl!("sound"),
                &edit.sound,
                Message::EditSound,
                Message::BrowseCustomSound,
            ));

            // Save/Cancel
            col = col.push(widget::divider::horizontal::default());
            let actions = widget::row::with_capacity(2)
                .spacing(8)
                .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEdit))
                .push(widget::button::suggested(fl!("save")).on_press(Message::SaveAlarm));
            col = col.push(actions);
        }

        col.into()
    }
}
