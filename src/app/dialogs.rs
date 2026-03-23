// SPDX-License-Identifier: MIT
//
// Dialog and settings view helpers for `AppModel`.

use super::{AppModel, ConfirmationCategory, DestructiveAction, Message};
use crate::fl;
use cosmic::iced::Length;
use cosmic::prelude::*;
use cosmic::widget;

impl AppModel {
    pub(super) fn settings_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(12).spacing(spacing);

        col = col.push(widget::text::body(fl!("time-format")));

        let btn_24h = if self.use_12h {
            widget::button::standard(fl!("time-format-24h")).on_press(Message::SetTimeFormat(false))
        } else {
            widget::button::suggested(fl!("time-format-24h"))
                .on_press(Message::SetTimeFormat(false))
        };
        let btn_12h = if self.use_12h {
            widget::button::suggested(fl!("time-format-12h")).on_press(Message::SetTimeFormat(true))
        } else {
            widget::button::standard(fl!("time-format-12h")).on_press(Message::SetTimeFormat(true))
        };

        let row = widget::row::with_capacity(2)
            .spacing(8)
            .push(btn_24h)
            .push(btn_12h);
        col = col.push(row);

        col = col.push(widget::divider::horizontal::default());
        col = col.push(widget::text::title4(fl!("settings-section-confirmation-dialogs")));

        col = col.push(
            widget::checkbox(self.confirm_delete_alarm)
                .label(fl!("settings-confirm-delete-alarm"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::DeleteAlarm, v)
                }),
        );
        col = col.push(
            widget::checkbox(self.confirm_delete_timer)
                .label(fl!("settings-confirm-delete-timer"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::DeleteTimer, v)
                }),
        );
        col = col.push(
            widget::checkbox(self.confirm_delete_world_clock)
                .label(fl!("settings-confirm-delete-world-clock"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::DeleteWorldClock, v)
                }),
        );
        col = col.push(
            widget::checkbox(self.confirm_delete_pomodoro)
                .label(fl!("settings-confirm-delete-pomodoro"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::DeletePomodoro, v)
                }),
        );
        col = col.push(
            widget::checkbox(self.confirm_clear_stopwatch)
                .label(fl!("settings-confirm-clear-stopwatch"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::ClearStopwatch, v)
                }),
        );

        col.into()
    }

    pub(super) fn shortcuts_dialog_view(&self) -> Element<'_, Message> {
        let spacing = 10;
        let mut col = widget::column::with_capacity(26).spacing(spacing);

        // Global shortcuts
        col = col.push(widget::text::title4(fl!("shortcuts-global")));
        col = col.push(Self::shortcut_row(fl!("shortcuts-quit"), &["Ctrl", "Q"]));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-next-tab"),
            &["Ctrl", "↓"],
        ));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-prev-tab"),
            &["Ctrl", "↑"],
        ));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-show-shortcuts"),
            &["Ctrl", "?"],
        ));

        col = col.push(widget::divider::horizontal::default());

        // Tab shortcuts
        col = col.push(widget::text::title4(fl!("shortcuts-tabs")));
        col = col.push(Self::shortcut_row(fl!("nav-world-clocks"), &["Alt", "1"]));
        col = col.push(Self::shortcut_row(fl!("nav-stopwatch"), &["Alt", "2"]));
        col = col.push(Self::shortcut_row(fl!("nav-alarm"), &["Alt", "3"]));
        col = col.push(Self::shortcut_row(fl!("nav-timer"), &["Alt", "4"]));
        col = col.push(Self::shortcut_row(fl!("nav-pomodoro"), &["Alt", "5"]));

        col = col.push(widget::divider::horizontal::default());

        // Page shortcuts
        col = col.push(widget::text::title4(fl!("shortcuts-page")));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-start-pause"),
            &["Space"],
        ));
        col = col.push(Self::shortcut_row(fl!("shortcuts-lap"), &["Enter"]));
        col = col.push(Self::shortcut_row(fl!("shortcuts-reset"), &["Delete"]));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-new-item"),
            &["Ctrl", "N"],
        ));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-skip-break"),
            &["Ctrl", "S"],
        ));

        let dialog = widget::dialog()
            .title(fl!("shortcuts"))
            .body(fl!("shortcuts-description"))
            .control(col)
            .primary_action(
                widget::button::standard(fl!("shortcuts-close"))
                    .on_press(Message::CloseShortcutsDialog),
            );

        dialog.into()
    }

    pub(super) fn confirmation_dialog_view(&self) -> Element<'_, Message> {
        let (title, body, confirm_label) = match &self.pending_destructive_action {
            Some(DestructiveAction::DeleteAlarm(_)) => (
                fl!("confirm-delete-alarm-title"),
                fl!("confirm-delete-alarm-body"),
                fl!("confirm-button-delete"),
            ),
            Some(DestructiveAction::DeleteTimer(_)) => (
                fl!("confirm-delete-timer-title"),
                fl!("confirm-delete-timer-body"),
                fl!("confirm-button-delete"),
            ),
            Some(DestructiveAction::DeleteWorldClock(_)) => (
                fl!("confirm-delete-world-clock-title"),
                fl!("confirm-delete-world-clock-body"),
                fl!("confirm-button-delete"),
            ),
            Some(DestructiveAction::DeletePomodoro(_)) => (
                fl!("confirm-delete-pomodoro-title"),
                fl!("confirm-delete-pomodoro-body"),
                fl!("confirm-button-delete"),
            ),
            Some(DestructiveAction::ClearStopwatchHistory) => (
                fl!("confirm-clear-stopwatch-title"),
                fl!("confirm-clear-stopwatch-body"),
                fl!("confirm-button-clear"),
            ),
            None => return widget::text::body("").into(),
        };

        let dont_show = widget::checkbox(self.confirm_dialog_dont_show_again)
            .label(fl!("confirm-dont-show-again"))
            .on_toggle(Message::ToggleConfirmDontShowAgain);

        widget::dialog()
            .title(title)
            .body(body)
            .control(dont_show)
            .primary_action(
                widget::button::destructive(confirm_label)
                    .on_press(Message::ConfirmDestructiveAction),
            )
            .secondary_action(
                widget::button::standard(fl!("confirm-button-cancel"))
                    .on_press(Message::CancelDestructiveAction),
            )
            .into()
    }

    pub(super) fn shortcut_row<'a>(action: String, keys: &'a [&'a str]) -> Element<'a, Message> {
        use cosmic::iced::widget::container as iced_container;
        use cosmic::iced_core::{Background, Border};

        let keys_row = keys.iter().fold(
            widget::row::with_capacity(keys.len() * 2)
                .spacing(4)
                .align_y(cosmic::iced::Alignment::Center),
            |row, key| {
                row.push(
                    widget::container(
                        widget::text::body(key.to_string())
                            .size(13.0)
                            .align_x(cosmic::iced::Alignment::Center),
                    )
                    .padding([2, 8])
                    .style(|theme: &cosmic::Theme| {
                        let cosmic = theme.cosmic();
                        iced_container::Style {
                            background: Some(Background::Color(
                                cosmic.background.component.hover.into(),
                            )),
                            border: Border {
                                color: cosmic.background.component.divider.into(),
                                width: 1.0,
                                radius: cosmic.corner_radii.radius_xs.into(),
                            },
                            ..Default::default()
                        }
                    }),
                )
            },
        );

        widget::row::with_capacity(2)
            .push(widget::text::body(action).width(Length::FillPortion(3)))
            .push(
                widget::container(keys_row)
                    .width(Length::FillPortion(2))
                    .align_x(cosmic::iced::Alignment::End),
            )
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }
}
