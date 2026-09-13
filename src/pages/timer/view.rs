// SPDX-License-Identifier: MIT
//
// Timer view functions: card grid, edit mode, empty state, and sidebar form.

use super::Message;
use super::model::*;
use crate::components::reorder_list::ReorderList;
use crate::components::{
    CircularProgress, TimeUnit, format_duration_hms, sound_selector_view, time_picker,
};
use crate::fl;
use cosmic::iced::font::Weight;
use cosmic::iced::{Alignment, Color, Length};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::Duration;

/// Font weight 300 (Light) for the timer time display inside the circle.
fn light_font() -> cosmic::iced::Font {
    cosmic::iced::Font {
        weight: Weight::Light,
        ..cosmic::font::default()
    }
}

impl TimerState {
    /// Main view: dispatches to focus mode, card grid, edit mode, or empty state.
    pub fn view(&self) -> Element<'_, Message> {
        // Focus mode wins over every other mode. The id is re-looked-up rather
        // than trusted: if it no longer resolves (deleted elsewhere) we fall back
        // to the list instead of rendering a blank page.
        if let Some(id) = self.focused_id
            && let Some(timer) = self.timers.iter().find(|t| t.id == id)
        {
            return self.focus_view(timer);
        }
        if self.edit_mode {
            self.edit_mode_view()
        } else {
            self.card_grid_view()
        }
    }

    /// Full-page view of a single timer: back header, large ring, controls.
    fn focus_view<'a>(&'a self, timer: &'a TimerEntry) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();

        let back = widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
            .on_press(Message::Unfocus);

        let header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(back)
            .push(
                widget::container(widget::text::title3(&timer.label))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
            // Balances the back button so the title stays optically centred.
            .push(widget::Space::new().width(40.0));

        let progress = if timer.initial_duration.as_secs_f32() > 0.0 {
            1.0 - (timer.remaining.as_secs_f32() / timer.initial_duration.as_secs_f32())
        } else {
            0.0
        };

        let circle_size = 280.0;
        let circle = CircularProgress::new(progress)
            .size(circle_size)
            .stroke_width(10.0)
            .track_color(Color::from_rgba(0.5, 0.5, 0.5, 0.15))
            .fill_color(cosmic::theme::active().cosmic().accent_color().into())
            .view();

        let time_text = widget::text(format_duration_hms(timer.remaining))
            .size(56.0)
            .font(light_font());

        let hero = widget::container(
            cosmic::iced_widget::stack![
                circle,
                widget::container(time_text)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(circle_size)
                    .height(circle_size),
            ]
            .width(circle_size)
            .height(circle_size),
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill);

        widget::column::with_capacity(3)
            .spacing(spacing.space_s)
            .padding(spacing.space_xs)
            .push(header)
            .push(hero)
            .push(self.card_controls(timer))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Card grid view (base/view mode): page header + timer card grid or empty state.
    fn card_grid_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let mut col = widget::column::with_capacity(3)
            .spacing(spacing.space_s)
            .width(Length::Fill)
            .height(Length::Fill);

        col = col.push(self.header_row());

        if self.timers.is_empty() {
            col = col.push(self.empty_state());
        } else {
            let cards: Vec<Element<'_, Message>> = self
                .timers
                .iter()
                .map(|timer| self.timer_card(timer))
                .collect();

            let grid = widget::flex_row(cards)
                .spacing(spacing.space_s as u16)
                .min_item_width(280.0)
                .width(Length::Fill);

            col = col.push(grid);
        }

        col.into()
    }

    /// A single timer card with circular progress, time display, and controls.
    fn timer_card<'a>(&'a self, timer: &'a TimerEntry) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();
        let id = timer.id;
        let progress = if timer.initial_duration.as_secs_f32() > 0.0 {
            1.0 - (timer.remaining.as_secs_f32() / timer.initial_duration.as_secs_f32())
        } else {
            0.0
        };

        let accent = cosmic::theme::active().cosmic().accent_color();
        let track_color = Color::from_rgba(0.5, 0.5, 0.5, 0.15);

        // Circular progress with time overlay
        let circle_size = 170.0;
        let circle = CircularProgress::new(progress)
            .size(circle_size)
            .stroke_width(6.0)
            .track_color(track_color)
            .fill_color(accent.into())
            .view();

        let remaining_str = format_duration_hms(timer.remaining);
        let time_text = widget::text(remaining_str).size(28.0).font(light_font());

        // Stack time text on top of circle using a layered container
        let circle_with_time = widget::container(
            cosmic::iced_widget::stack![
                circle,
                widget::container(time_text)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(circle_size)
                    .height(circle_size),
            ]
            .width(circle_size)
            .height(circle_size),
        )
        .align_x(Alignment::Center)
        .width(Length::Fill);

        // Label
        let label = widget::container(widget::text::title4(&timer.label))
            .align_x(Alignment::Center)
            .width(Length::Fill);

        // Repeat info
        let repeat_info: Option<Element<'a, Message>> = if timer.repeat_enabled {
            let repeat_str = if timer.repeat_count == 0 {
                fl!(
                    "repeat-progress-infinite",
                    completed = timer.completed_count.to_string()
                )
            } else {
                fl!(
                    "repeat-progress",
                    completed = timer.completed_count.to_string(),
                    total = timer.repeat_count.to_string()
                )
            };
            Some(
                widget::container(widget::text::caption(repeat_str))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .into(),
            )
        } else {
            None
        };

        // Controls: reset (left) + play/pause (center)
        let controls = self.card_controls(timer);

        // Build card column
        let mut card_col = widget::column::with_capacity(5)
            .spacing(spacing.space_xxs)
            .align_x(Alignment::Center)
            .padding(spacing.space_s);

        card_col = card_col.push(circle_with_time);
        card_col = card_col.push(label);
        if let Some(info) = repeat_info {
            card_col = card_col.push(info);
        }
        card_col = card_col.push(controls);

        // Card container with themed background and fixed width
        let card = widget::container(card_col)
            .width(Length::Fill)
            .max_width(340.0)
            .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                let mut style = cosmic::iced_widget::container::Catalog::style(
                    theme,
                    &cosmic::theme::Container::Primary,
                );
                style.border.radius = theme.cosmic().radius_s().into();
                style.background = Some(Color::from(theme.cosmic().bg_component_color()).into());
                style
            })));

        // Pressing the card enters focus mode, mirroring World Clocks' detail
        // view. Editing moved to the pencil in the controls row and to edit mode.
        widget::mouse_area(card).on_press(Message::Focus(id)).into()
    }

    /// Play/Pause and Reset controls for a timer card.
    /// Mirrors the stopwatch button pattern but slightly smaller (48px primary, 32px secondary).
    fn card_controls(&self, timer: &TimerEntry) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let id = timer.id;
        let has_started = timer.is_running || timer.remaining < timer.initial_duration;

        // Primary button: play/pause
        let (primary_icon, primary_tooltip, primary_msg, use_accent) = if timer.is_running {
            (
                "media-playback-pause-symbolic",
                fl!("tooltip-pause"),
                Message::PauseTimer(id),
                false,
            )
        } else if timer.remaining == Duration::ZERO {
            // Timer completed — show reset-style play to restart
            (
                "media-playback-start-symbolic",
                fl!("tooltip-start"),
                Message::ResetTimer(id),
                true,
            )
        } else if timer.remaining < timer.initial_duration {
            (
                "media-playback-start-symbolic",
                fl!("tooltip-resume"),
                Message::ResumeTimer(id),
                true,
            )
        } else {
            (
                "media-playback-start-symbolic",
                fl!("tooltip-start"),
                Message::StartTimer(id),
                true,
            )
        };

        let primary_btn_inner = widget::icon::from_name(primary_icon).size(24).icon();
        let primary_btn = widget::tooltip(
            widget::button::custom(
                widget::container(primary_btn_inner)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(48)
                    .height(48),
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

        // Reset button: only visible when timer has been started
        let reset_btn: Option<Element<'_, Message>> = if has_started {
            let icon = widget::icon::from_name("edit-undo-symbolic")
                .size(16)
                .icon();
            Some(
                widget::tooltip(
                    widget::button::custom(
                        widget::container(icon)
                            .align_x(Alignment::Center)
                            .align_y(Alignment::Center)
                            .width(32)
                            .height(32),
                    )
                    .class(cosmic::theme::Button::Standard)
                    .on_press(Message::ResetTimer(id)),
                    widget::text::body(fl!("tooltip-reset")),
                    widget::tooltip::Position::Top,
                )
                .into(),
            )
        } else {
            None
        };

        // Fixed layout: primary centered, reset on right
        // Use invisible spacers to keep primary centered
        let invisible_btn = || -> Element<'_, Message> {
            widget::button::custom(
                widget::container(widget::Space::new())
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(32)
                    .height(32),
            )
            .class(cosmic::theme::Button::Custom {
                active: Box::new(|_, _| widget::button::Style::default()),
                disabled: Box::new(|_| widget::button::Style::default()),
                hovered: Box::new(|_, _| widget::button::Style::default()),
                pressed: Box::new(|_, _| widget::button::Style::default()),
            })
            .into()
        };

        // Edit affordance, now that pressing the card focuses instead of editing.
        // Only offered when stopped: saving an edit resets the timer's duration.
        let edit_btn: Option<Element<'_, Message>> = (!timer.is_running).then(|| {
            widget::tooltip(
                widget::button::custom(
                    widget::container(
                        widget::icon::from_name("edit-symbolic").size(16).icon(),
                    )
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(32)
                    .height(32),
                )
                .class(cosmic::theme::Button::Standard)
                .on_press(Message::StartEditTimer(id)),
                widget::text::body(fl!("edit-timer")),
                widget::tooltip::Position::Top,
            )
            .into()
        });

        let left_spacer: Element<'_, Message> = edit_btn.unwrap_or_else(invisible_btn);
        let right_slot: Element<'_, Message> = reset_btn.unwrap_or_else(invisible_btn);

        widget::container(
            widget::row::with_capacity(3)
                .spacing(spacing.space_s)
                .align_y(Alignment::Center)
                .push(left_spacer)
                .push(primary_btn)
                .push(right_slot),
        )
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .into()
    }

    /// Edit mode view: card list with delete buttons and drag handles.
    fn edit_mode_view(&self) -> Element<'_, Message> {
        let cosmic::cosmic_theme::Spacing {
            space_xxs,
            space_xs,
            space_xxxs,
            ..
        } = cosmic::theme::spacing();

        let mut col = widget::column::with_capacity(self.timers.len() + 3).spacing(space_xxs);

        col = col.push(self.header_row());

        if self.timers.is_empty() {
            col = col.push(self.empty_state());
        } else {
            let dragging = self.dragging_index;

            let card_rows: Vec<Element<'_, Message>> = self
                .timers
                .iter()
                .enumerate()
                .map(|(i, timer)| {
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

                    let id = timer.id;
                    let remaining_str = format_duration_hms(timer.remaining);

                    let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(5);

                    // Drag handle
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

                    // Timer info: label + time + repeat
                    let mut info_col = widget::column::with_capacity(3).spacing(space_xxxs);
                    info_col = info_col.push(widget::text::body(&timer.label));
                    info_col = info_col.push(widget::text::title4(remaining_str));
                    if timer.repeat_enabled {
                        let repeat_str = if timer.repeat_count == 0 {
                            fl!(
                                "repeat-progress-infinite",
                                completed = timer.completed_count.to_string()
                            )
                        } else {
                            fl!(
                                "repeat-progress",
                                completed = timer.completed_count.to_string(),
                                total = timer.repeat_count.to_string()
                            )
                        };
                        info_col = info_col.push(widget::text::caption(repeat_str));
                    }
                    items.push(info_col.width(Length::Fill).into());

                    // Delete button
                    items.push(
                        widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                            .extra_small()
                            .tooltip(fl!("tooltip-delete"))
                            .on_press(Message::DeleteTimer(id))
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

            let item_count = self.timers.len();

            // Pre-clone timer data for drag icon builder
            let timers_snapshot: Vec<(String, String)> = self
                .timers
                .iter()
                .map(|t| (t.label.clone(), format_duration_hms(t.remaining)))
                .collect();

            let reorder_list = ReorderList::new(cards, item_count, self.dragging_index)
                .on_start_drag(Message::StartDrag)
                .on_reorder(|from, to| Message::Reorder(from, to))
                .on_finish(Message::FinishDrag)
                .on_cancel(Message::CancelDrag)
                .drag_icon(move |index, offset| {
                    let (label, time_str) = timers_snapshot
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| ("Timer".to_string(), String::new()));

                    let content = widget::row::with_children(vec![
                        widget::icon::from_name("grip-lines-symbolic")
                            .size(16)
                            .icon()
                            .into(),
                        widget::column::with_capacity(2)
                            .push(widget::text::body(label))
                            .push(widget::text::title4(time_str))
                            .width(Length::Fill)
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

        col.into()
    }

    /// Shared header row: title + edit button (if timers exist) + add button.
    fn header_row(&self) -> Element<'_, Message> {
        let mut header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("timer-title")).width(Length::Fill));

        // Only show edit button when there are timers to edit
        if !self.timers.is_empty() {
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
                .on_press(Message::StartNew),
        );

        header.into()
    }

    /// Shared empty state: centered timer icon + CTA button.
    fn empty_state(&self) -> Element<'_, Message> {
        let icon = widget::icon::icon(crate::app::bundled_icon(crate::app::TIMER_ICON))
            .size(128)
            .class(cosmic::theme::Svg::Custom(std::rc::Rc::new(
                |theme: &cosmic::Theme| cosmic::iced_widget::svg::Style {
                    color: Some(theme.cosmic().palette.neutral_5.into()),
                },
            )));

        let empty_state = widget::column::with_capacity(2)
            .spacing(16)
            .align_x(Alignment::Center)
            .push(icon)
            .push(widget::button::suggested(fl!("create-timer")).on_press(Message::StartNew));

        widget::container(empty_state)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Sidebar view: timer creation/editing form
    pub fn sidebar_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(8).spacing(spacing);

        // Label
        col = col.push(widget::text::body(fl!("label")));
        col = col.push(
            widget::text_input(fl!("timer-label-placeholder"), &self.edit_label)
                .id(widget::Id::new("timer-label-input"))
                .on_input(Message::EditLabel),
        );

        // Duration spinners with wrap-around (compact vertical HH:MM:SS steppers)
        col = col.push(widget::text::body(fl!("duration")));

        let h = self.edit_hours;
        let m = self.edit_minutes;
        let s = self.edit_seconds;

        col = col.push(time_picker(vec![
            TimeUnit::new(
                format!("{:02}", h),
                Message::EditHours((h + 1) % 24),
                Message::EditHours(if h == 0 { 23 } else { h - 1 }),
            ),
            TimeUnit::new(
                format!("{:02}", m),
                Message::EditMinutes((m + 1) % 60),
                Message::EditMinutes(if m == 0 { 59 } else { m - 1 }),
            ),
            TimeUnit::new(
                format!("{:02}", s),
                Message::EditSeconds((s + 1) % 60),
                Message::EditSeconds(if s == 0 { 59 } else { s - 1 }),
            ),
        ]));

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

        let mut actions = widget::row::with_capacity(3).spacing(8);

        // Delete button for existing timers
        if let Some(id) = self.edit_id {
            actions = actions.push(
                widget::button::destructive(fl!("delete")).on_press(Message::DeleteTimer(id)),
            );
        }

        let save_label = if self.edit_id.is_some() {
            fl!("save")
        } else {
            fl!("add-timer")
        };
        actions = actions
            .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEdit))
            .push(widget::button::suggested(save_label).on_press(Message::SaveTimer));
        col = col.push(actions);

        col.into()
    }
}
