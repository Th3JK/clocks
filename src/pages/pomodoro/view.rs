// SPDX-License-Identifier: MIT
//
// Pomodoro view functions: main page view and settings sidebar.

use super::model::*;
use super::Message;
use crate::components::{format_duration, sound_selector_view};
use crate::fl;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;

impl PomodoroState {
    /// Main view: page header + all pomodoro timers
    pub fn view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(self.timers.len() * 5 + 2).spacing(spacing);

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("pomodoro-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .tooltip(fl!("tooltip-add"))
                    .on_press(Message::OpenSettings),
            );
        col = col.push(header);

        if self.timers.is_empty() {
            col = col.push(
                widget::container(widget::text::body(fl!("no-pomodoro-timers")))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .padding(24),
            );
        }

        for timer in &self.timers {
            let id = timer.id;

            col = col.push(widget::divider::horizontal::default());

            // Label row with Edit/Delete aligned right
            let mut label_row = widget::row::with_capacity(3)
                .spacing(spacing)
                .align_y(Alignment::Center)
                .push(
                    widget::text::title4(&timer.label).width(Length::Fill),
                );
            if !timer.is_running {
                label_row = label_row.push(
                    widget::button::icon(widget::icon::from_name("edit-symbolic"))
                        .tooltip(fl!("tooltip-edit"))
                        .on_press(Message::StartEditPomodoro(id)),
                );
            }
            label_row = label_row.push(
                widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                    .tooltip(fl!("tooltip-delete"))
                    .on_press(Message::Delete(id)),
            );
            col = col.push(label_row);

            // Session info
            let session_info = fl!("session-info",
                number = timer.session_number.to_string(),
                session_type = timer.session_type.display_name()
            );
            col = col.push(widget::text::caption(session_info));

            // Time display
            col = col.push(
                widget::container(widget::text::title1(format_duration(timer.remaining)))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .padding(12),
            );

            // Controls (without Edit/Delete, those are now in the label row)
            let mut controls = widget::row::with_capacity(3)
                .spacing(spacing)
                .align_y(Alignment::Center);

            if timer.is_running {
                controls =
                    controls.push(widget::button::standard(fl!("pause")).on_press(Message::Pause(id)));
            } else if timer.remaining < timer.started_remaining
                || (timer.completed_work_sessions > 0 && timer.session_type == SessionType::Work)
            {
                controls = controls
                    .push(widget::button::suggested(fl!("resume")).on_press(Message::Resume(id)));
            } else {
                controls =
                    controls.push(widget::button::suggested(fl!("start")).on_press(Message::Start(id)));
            }

            controls = controls.push(widget::button::standard(fl!("skip")).on_press(Message::Skip(id)));
            controls =
                controls.push(widget::button::destructive(fl!("reset")).on_press(Message::Reset(id)));

            col = col.push(
                widget::container(controls)
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            );

            // Progress
            let progress_text = fl!("progress-info",
                completed = timer.completed_work_sessions.to_string(),
                target = timer.target_sessions.to_string(),
                focused = (timer.total_focused_secs / 60).to_string()
            );
            col = col.push(
                widget::container(widget::text::caption(progress_text))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            );
        }

        col.into()
    }

    /// Settings sidebar view
    pub fn settings_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(14).spacing(spacing);

        if let Some(_edit_id) = self.editing_id {
            // Editing existing pomodoro timer
            col = col.push(widget::text::title4(fl!("edit-pomodoro")));
            col = col.push(
                widget::text_input(fl!("label"), &self.edit_label)
                    .id(widget::Id::new("pomodoro-label-input"))
                    .on_input(Message::EditNewLabel),
            );

            let w = self.edit_work_minutes;
            let work_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("work-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultWorkMinutes(w.saturating_sub(5))),
                )
                .push(widget::text::body(fl!("minutes-value", value = w.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultWorkMinutes(w + 5)),
                );
            col = col.push(work_row);

            let sb = self.edit_short_break_minutes;
            let short_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("short-break-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultShortBreakMinutes(sb.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = sb.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultShortBreakMinutes(sb + 1)),
                );
            col = col.push(short_row);

            let lb = self.edit_long_break_minutes;
            let long_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("long-break-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultLongBreakMinutes(lb.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = lb.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultLongBreakMinutes(lb + 1)),
                );
            col = col.push(long_row);

            // Sound selection
            col = col.push(widget::divider::horizontal::default());
            col = col.push(sound_selector_view(
                fl!("sound"),
                &self.edit_sound,
                Message::EditSound,
                Message::BrowseCustomSound,
            ));

            col = col.push(widget::divider::horizontal::default());
            let actions = widget::row::with_capacity(2)
                .spacing(8)
                .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEditPomodoro))
                .push(widget::button::suggested(fl!("save")).on_press(Message::SaveEditPomodoro));
            col = col.push(actions);
        } else {
            // Add new timer
            col = col.push(widget::text::title4(fl!("new-pomodoro")));
            col = col.push(
                widget::text_input(fl!("label-placeholder-pomodoro"), &self.edit_label)
                    .id(widget::Id::new("pomodoro-label-input"))
                    .on_input(Message::EditNewLabel),
            );
            col = col.push(widget::button::suggested(fl!("add-timer")).on_press(Message::AddTimer));

            col = col.push(widget::divider::horizontal::default());

            // Default durations for new timers
            col = col.push(widget::text::title4(fl!("default-durations")));

            let w = self.default_work_minutes;
            let work_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("work-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultWorkMinutes(w.saturating_sub(5))),
                )
                .push(widget::text::body(fl!("minutes-value", value = w.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultWorkMinutes(w + 5)),
                );
            col = col.push(work_row);

            let sb = self.default_short_break_minutes;
            let short_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("short-break-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultShortBreakMinutes(sb.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = sb.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultShortBreakMinutes(sb + 1)),
                );
            col = col.push(short_row);

            let lb = self.default_long_break_minutes;
            let long_row = widget::row::with_capacity(4)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(fl!("long-break-label")).width(Length::Fixed(100.0)))
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))

                        .on_press(Message::SetDefaultLongBreakMinutes(lb.saturating_sub(1))),
                )
                .push(widget::text::body(fl!("minutes-value", value = lb.to_string())))
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))

                        .on_press(Message::SetDefaultLongBreakMinutes(lb + 1)),
                );
            col = col.push(long_row);
        }

        col.into()
    }
}
