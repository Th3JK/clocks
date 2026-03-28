// SPDX-License-Identifier: MIT
//
// Alarm view functions: main page view and sidebar editing form.

use super::model::*;
use super::update::hour24_to_12;
use super::Message;
use crate::components::reorder_list::ReorderList;
use crate::components::sound_selector_view;
use crate::fl;
use cosmic::iced::{Alignment, Color, Length};
use cosmic::prelude::*;
use cosmic::widget;

impl AlarmState {
    /// Main view: dispatches to empty state, list view, or edit mode view.
    pub fn view(&self, use_12h: bool, auto_sort: bool) -> Element<'_, Message> {
        if self.edit_mode {
            self.edit_mode_view(use_12h, auto_sort)
        } else {
            self.list_view(use_12h)
        }
    }

    /// List view (base/view mode): header + alarm list or empty state.
    /// Rows have no delete icon, a right-facing chevron, and full-width click area.
    fn list_view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(self.alarms.len() + 3).spacing(spacing);

        col = col.push(self.header_row());

        if self.alarms.is_empty() {
            col = col.push(self.empty_state());
        } else {
            let mut list_col = widget::list_column();

            for alarm in &self.alarms {
                let time_str = Self::format_alarm_time(alarm, use_12h);
                let id = alarm.id;

                // Left side: label + time + repeat info
                let left = widget::column::with_capacity(3)
                    .push(widget::text::body(&alarm.label))
                    .push(widget::text::title3(time_str))
                    .push(widget::text::caption(format!("{}", alarm.repeat_mode)))
                    .width(Length::Fill);

                // Toggle
                let toggle = widget::toggler(alarm.is_enabled)
                    .on_toggle(move |_| Message::ToggleAlarm(id));

                // Chevron
                let chevron = widget::icon::from_name("go-next-symbolic")
                    .size(16)
                    .icon();

                let clickable = widget::row::with_capacity(3)
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(left)
                    .push(toggle)
                    .push(chevron);

                let row = widget::mouse_area(
                    widget::container(clickable)
                        .width(Length::Fill)
                        .padding([8, 0]),
                )
                .on_press(Message::StartEditAlarm(id));

                list_col = list_col.add(row);
            }

            col = col.push(list_col);
        }

        col.into()
    }

    /// Edit mode view: card rows with delete buttons.
    /// When `auto_sort` is false, drag handles and ReorderList are shown for manual reordering.
    fn edit_mode_view(&self, use_12h: bool, auto_sort: bool) -> Element<'_, Message> {
        let cosmic::cosmic_theme::Spacing {
            space_xxxs,
            space_xxs,
            space_xs,
            ..
        } = cosmic::theme::spacing();

        let mut col = widget::column::with_capacity(self.alarms.len() + 3).spacing(space_xxs);

        col = col.push(self.header_row());

        if self.alarms.is_empty() {
            col = col.push(self.empty_state());
        } else {
            let dragging = if auto_sort { None } else { self.dragging_index };

            let card_rows: Vec<Element<'_, Message>> = self
                .alarms
                .iter()
                .enumerate()
                .map(|(i, alarm)| {
                    // Collapse the dragged item to an accent-colored drop indicator line
                    if dragging == Some(i) {
                        return widget::container(widget::Space::new().width(Length::Fill))
                            .height(Length::Fixed(4.0))
                            .width(Length::Fill)
                            .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                                let accent = Color::from(theme.cosmic().accent_color());
                                cosmic::iced_widget::container::Style {
                                    background: Some(cosmic::iced::Background::Color(accent)),
                                    border: cosmic::iced::Border {
                                        radius: 2.0.into(),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                }
                            })))
                            .into();
                    }

                    let time_str = Self::format_alarm_time(alarm, use_12h);
                    let id = alarm.id;

                    // Row content: [drag handle] | alarm info | delete button
                    let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(4);

                    // Drag handle — only when manual ordering
                    if !auto_sort {
                        items.push(
                            widget::icon::from_name("grip-lines-symbolic")
                                .size(16)
                                .icon()
                                .class(cosmic::theme::Svg::Custom(std::rc::Rc::new(
                                    |theme: &cosmic::Theme| cosmic::iced_widget::svg::Style {
                                        color: Some(theme.cosmic().palette.neutral_7.into()),
                                    },
                                )))
                                .into(),
                        );
                    }

                    // Alarm icon
                    items.push(
                        widget::icon::from_name("alarm-symbolic")
                            .size(20)
                            .icon()
                            .into(),
                    );

                    // Text block: label (primary) + time + repeat (secondary)
                    items.push(
                        widget::column::with_capacity(3)
                            .spacing(space_xxxs)
                            .width(Length::Fill)
                            .push(widget::text::body(&alarm.label))
                            .push(widget::text::title4(time_str))
                            .push(widget::text::caption(format!("{}", alarm.repeat_mode)))
                            .into(),
                    );

                    // Delete button
                    items.push(
                        widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                            .extra_small()
                            .tooltip(fl!("tooltip-delete"))
                            .on_press(Message::DeleteAlarm(id))
                            .into(),
                    );

                    let content = widget::row::with_children(items)
                        .spacing(space_xs)
                        .align_y(Alignment::Center);

                    // Card container
                    widget::container(content)
                        .padding(8)
                        .width(Length::Fill)
                        .class(cosmic::theme::Container::Custom(Box::new(move |theme| {
                            let mut style = cosmic::iced_widget::container::Catalog::style(
                                theme,
                                &cosmic::theme::Container::Primary,
                            );
                            style.border.radius = theme.cosmic().radius_s().into();
                            style.background =
                                Some(Color::from(theme.cosmic().bg_component_color()).into());
                            style
                        })))
                        .into()
                })
                .collect();

            let cards = widget::column::with_children(card_rows).spacing(space_xxs);

            if auto_sort {
                // No drag-to-reorder when auto-sorting
                col = col.push(cards);
            } else {
                let item_count = self.alarms.len();

                // Pre-clone alarm data for the drag icon builder ('static closure)
                let alarms_snapshot: Vec<(String, String, String)> = self
                    .alarms
                    .iter()
                    .map(|alarm| {
                        let time_str = Self::format_alarm_time(alarm, use_12h);
                        (
                            alarm.label.clone(),
                            time_str,
                            format!("{}", alarm.repeat_mode),
                        )
                    })
                    .collect();

                let reorder_list = ReorderList::new(cards, item_count, self.dragging_index)
                    .on_start_drag(Message::StartDrag)
                    .on_reorder(|from, to| Message::Reorder(from, to))
                    .on_finish(Message::FinishDrag)
                    .on_cancel(Message::CancelDrag)
                    .drag_icon(move |index, offset| {
                        let (label, time_str, repeat_str) = alarms_snapshot
                            .get(index)
                            .cloned()
                            .unwrap_or_else(|| {
                                ("Alarm".to_string(), String::new(), String::new())
                            });

                        let content = widget::row::with_children(vec![
                            widget::icon::from_name("grip-lines-symbolic")
                                .size(16)
                                .icon()
                                .into(),
                            widget::icon::from_name("alarm-symbolic")
                                .size(20)
                                .icon()
                                .into(),
                            widget::column::with_capacity(3)
                                .spacing(space_xxxs)
                                .width(Length::Fill)
                                .push(widget::text::body(label))
                                .push(widget::text::title4(time_str))
                                .push(widget::text::caption(repeat_str))
                                .into(),
                        ])
                        .spacing(space_xs)
                        .align_y(Alignment::Center);

                        // Card with accent border for the floating drag icon
                        let card: Element<'static, ()> = widget::container(content)
                            .padding(8)
                            .width(Length::Fill)
                            .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                                let accent = Color::from(theme.cosmic().accent_color());
                                let mut style = cosmic::iced_widget::container::Catalog::style(
                                    theme,
                                    &cosmic::theme::Container::Primary,
                                );
                                style.border.radius = theme.cosmic().radius_s().into();
                                style.border.color = accent;
                                style.border.width = 2.0;
                                style.background =
                                    Some(Color::from(theme.cosmic().bg_component_color()).into());
                                style
                            })))
                            .into();

                        (card, cosmic::iced_core::widget::tree::State::None, offset)
                    });

                col = col.push(reorder_list);
            }
        }

        col.into()
    }

    /// Shared header row: title + edit button (if alarms exist) + add button.
    fn header_row(&self) -> Element<'_, Message> {
        let mut header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("alarms-title")).width(Length::Fill));

        // Only show edit button when there are alarms to edit
        if !self.alarms.is_empty() {
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
                .on_press(Message::StartNewAlarm),
        );

        header.into()
    }

    /// Shared empty state: centered alarm icon + CTA button.
    fn empty_state(&self) -> Element<'_, Message> {
        let icon = widget::icon::from_name("alarm-symbolic")
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
                widget::button::suggested(fl!("create-alarm"))
                    .on_press(Message::StartNewAlarm),
            );

        widget::container(empty_state)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Format alarm time as a display string.
    fn format_alarm_time(alarm: &AlarmEntry, use_12h: bool) -> String {
        if use_12h {
            let (h12, is_pm) = hour24_to_12(alarm.hour);
            let period = if is_pm { fl!("pm") } else { fl!("am") };
            format!("{:02}:{:02} {}", h12, alarm.minute, period)
        } else {
            format!("{:02}:{:02}", alarm.hour, alarm.minute)
        }
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

            // Save/Cancel/Delete
            col = col.push(widget::divider::horizontal::default());
            let mut actions = widget::row::with_capacity(3)
                .spacing(8)
                .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEdit))
                .push(widget::button::suggested(fl!("save")).on_press(Message::SaveAlarm));
            if let Some(id) = edit.id {
                actions = actions.push(
                    widget::button::destructive(fl!("delete"))
                        .on_press(Message::DeleteAlarm(id)),
                );
            }
            col = col.push(actions);
        }

        col.into()
    }
}
