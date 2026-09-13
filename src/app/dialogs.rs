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

        // Three-way: System follows the desktop (falling back to the locale),
        // the other two are explicit. Selection is highlighted by swapping
        // suggested/standard, matching the rest of the app.
        let format_button = |label: String, value: crate::time_format::TimeFormat| {
            if self.time_format == value {
                widget::button::suggested(label).on_press(Message::SetTimeFormat(value))
            } else {
                widget::button::standard(label).on_press(Message::SetTimeFormat(value))
            }
        };

        let row = widget::row::with_capacity(3)
            .spacing(8)
            .push(format_button(
                fl!("time-format-system"),
                crate::time_format::TimeFormat::System,
            ))
            .push(format_button(
                fl!("time-format-24h"),
                crate::time_format::TimeFormat::TwentyFour,
            ))
            .push(format_button(
                fl!("time-format-12h"),
                crate::time_format::TimeFormat::Twelve,
            ));
        col = col.push(row);

        col = col.push(widget::divider::horizontal::default());
        col = col.push(widget::text::title4(fl!("settings-section-world-clocks")));
        col = col.push(
            widget::checkbox(self.auto_sort_world_clocks)
                .label(fl!("settings-auto-sort-world-clocks"))
                .on_toggle(Message::SetAutoSortWorldClocks),
        );

        col = col.push(widget::divider::horizontal::default());
        col = col.push(widget::text::title4(fl!("settings-section-alarms")));
        col = col.push(
            widget::checkbox(self.auto_sort_alarms)
                .label(fl!("settings-auto-sort-alarms"))
                .on_toggle(Message::SetAutoSortAlarms),
        );

        col = col.push(widget::divider::horizontal::default());
        col = col.push(widget::text::title4(fl!("settings-section-stopwatch")));
        col = col.push(
            widget::checkbox(self.auto_clear_stopwatch_history)
                .label(fl!("settings-auto-clear-stopwatch-history"))
                .on_toggle(Message::SetAutoClearStopwatchHistory),
        );

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

    /// Quick-action palette. Shows a live reading of what the current text
    /// would do, so the action is never a surprise on Enter.
    pub(super) fn palette_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let parsed = crate::quick_action::parse(&self.palette_input);
        let preview = match &parsed {
            Some(action) => self.describe_action(action),
            None if self.palette_input.trim().is_empty() => fl!("palette-hint"),
            None => fl!("palette-no-match"),
        };

        // Rows: whatever the text parses to first (so Enter and clicking the top
        // row agree), then the suggestions, filtered on their visible text.
        let query = self.palette_input.trim().to_lowercase();
        let mut rows: Vec<Element<'_, Message>> = Vec::new();
        if let Some(action) = &parsed {
            rows.push(self.palette_row(action, true));
        }
        for action in self.palette_suggestions() {
            if Some(&action) == parsed.as_ref() {
                continue;
            }
            if query.is_empty() || self.describe_action(&action).to_lowercase().contains(&query) {
                rows.push(self.palette_row(&action, false));
            }
        }

        let list = widget::scrollable(
            widget::column::with_children(rows)
                .spacing(spacing.space_xxxs)
                .width(Length::Fill),
        )
        .height(Length::Fixed(240.0));

        let control = widget::column::with_capacity(3)
            .spacing(spacing.space_xs)
            .push(
                widget::text_input(fl!("palette-placeholder"), &self.palette_input)
                    .id(widget::Id::new("palette-input"))
                    .on_input(Message::PaletteInput)
                    // Enter must come from the input: the global key subscription
                    // only sees keys no focused widget consumed, and a focused
                    // text input consumes Enter.
                    .on_submit(|_| Message::PaletteSubmit)
                    // Escape unfocuses the input and is captured there, so it
                    // never reaches the global handler — this is the only hook
                    // that sees it. It also fires on click-outside, which is why
                    // every button below carries its own payload rather than
                    // re-reading `palette_input`.
                    .on_unfocus(Message::ClosePalette),
            )
            .push(widget::text::caption(preview))
            .push(list);

        let mut dialog = widget::dialog()
            .title(fl!("palette-title"))
            .control(control)
            .secondary_action(
                widget::button::standard(fl!("cancel")).on_press(Message::ClosePalette),
            );

        // Only offer the confirm button when there is something to confirm. It
        // carries the parsed action rather than re-reading the input, because
        // pressing it unfocuses the input and that clears `palette_input` first.
        if let Some(action) = parsed {
            dialog = dialog.primary_action(
                widget::button::suggested(fl!("palette-run"))
                    .on_press(Message::PaletteRun(action)),
            );
        }

        dialog.into()
    }

    /// A clickable palette row. `primary` marks the parsed-from-text row so it
    /// reads as the thing Enter would do.
    fn palette_row(
        &self,
        action: &crate::quick_action::QuickAction,
        primary: bool,
    ) -> Element<'_, Message> {
        let label = self.describe_action(action);
        widget::button::custom(
            widget::container(widget::text::body(label))
                .padding(6)
                .width(Length::Fill),
        )
        .class(if primary {
            cosmic::theme::Button::Suggested
        } else {
            cosmic::theme::Button::Text
        })
        .width(Length::Fill)
        .on_press(Message::PaletteRun(action.clone()))
        .into()
    }

    /// One-line description of what an action will do.
    fn describe_action(&self, action: &crate::quick_action::QuickAction) -> String {
        use crate::quick_action::QuickAction;
        match action {
            QuickAction::Timer { secs, label } => fl!(
                "palette-preview-timer",
                duration = crate::components::format_duration_hms(std::time::Duration::from_secs(
                    *secs
                )),
                label = label.clone().unwrap_or_default()
            ),
            QuickAction::Alarm {
                hour,
                minute,
                label,
            } => fl!(
                "palette-preview-alarm",
                time = crate::time_format::format_hm(*hour, *minute, self.use_12h),
                label = label.clone().unwrap_or_default()
            ),
            QuickAction::Countdown {
                year,
                month,
                day,
                label,
            } => fl!(
                "palette-preview-countdown",
                date = format!("{year:04}-{month:02}-{day:02}"),
                label = label.clone().unwrap_or_default()
            ),
            QuickAction::Clock { query } => {
                fl!("palette-preview-clock", query = query.clone())
            }
            QuickAction::Navigate(page) => fl!(
                "palette-preview-navigate",
                page = self.page_title(*page)
            ),
            // Launchers carry only an id, so the label is looked up here — a row
            // should read "Start Morning HIIT", not "Start workout 3".
            QuickAction::StartTimer(id) => fl!(
                "palette-preview-start",
                label = self
                    .timer
                    .timers
                    .iter()
                    .find(|t| t.id == *id)
                    .map(|t| t.label.clone())
                    .unwrap_or_default()
            ),
            QuickAction::StartPomodoro(id) => fl!(
                "palette-preview-start",
                label = self
                    .pomodoro
                    .timers
                    .iter()
                    .find(|p| p.id == *id)
                    .map(|p| p.label.clone())
                    .unwrap_or_default()
            ),
            QuickAction::StartWorkout(id) => fl!(
                "palette-preview-start",
                label = self
                    .workout
                    .workouts
                    .iter()
                    .find(|w| w.id == *id)
                    .map(|w| w.label.clone())
                    .unwrap_or_default()
            ),
        }
    }

    /// The nav label for a page, so previews match the sidebar.
    fn page_title(&self, page: crate::pages::Page) -> String {
        use crate::pages::Page;
        match page {
            Page::WorldClocks => fl!("nav-world-clocks"),
            Page::Stopwatch => fl!("nav-stopwatch"),
            Page::Alarm => fl!("nav-alarm"),
            Page::Timer => fl!("nav-timer"),
            Page::Pomodoro => fl!("nav-pomodoro"),
            Page::Chess => fl!("nav-chess"),
            Page::Workout => fl!("nav-workout"),
            Page::Countdown => fl!("nav-countdown"),
        }
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
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-quick-action"),
            &["Ctrl", "K"],
        ));

        col = col.push(widget::divider::horizontal::default());

        // Tab shortcuts
        col = col.push(widget::text::title4(fl!("shortcuts-tabs")));
        col = col.push(Self::shortcut_row(fl!("nav-world-clocks"), &["Alt", "1"]));
        col = col.push(Self::shortcut_row(fl!("nav-stopwatch"), &["Alt", "2"]));
        col = col.push(Self::shortcut_row(fl!("nav-alarm"), &["Alt", "3"]));
        col = col.push(Self::shortcut_row(fl!("nav-timer"), &["Alt", "4"]));
        col = col.push(Self::shortcut_row(fl!("nav-pomodoro"), &["Alt", "5"]));
        col = col.push(Self::shortcut_row(fl!("nav-chess"), &["Alt", "6"]));
        col = col.push(Self::shortcut_row(fl!("nav-workout"), &["Alt", "7"]));
        col = col.push(Self::shortcut_row(fl!("nav-countdown"), &["Alt", "8"]));

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
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-chess-switch"),
            &["Space"],
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
