// SPDX-License-Identifier: MIT
//
// Countdown view: event cards with a live remaining/elapsed readout, plus the
// create/edit form in the context drawer.

use super::Message;
use super::model::*;
use crate::components::{TimeUnit, sound_selector_view, time_picker_row};
use crate::fl;
use chrono::Local;
use cosmic::iced::font::Weight;
use cosmic::iced::{Alignment, Color, Length};
use cosmic::prelude::*;
use cosmic::widget;

fn light_font() -> cosmic::iced::Font {
    cosmic::iced::Font {
        weight: Weight::Light,
        ..cosmic::font::default()
    }
}

/// Break a signed second count into days / hours / minutes / seconds.
fn dhms(total: i64) -> (i64, i64, i64, i64) {
    let t = total.abs();
    (t / 86_400, (t % 86_400) / 3600, (t % 3600) / 60, t % 60)
}

impl CountdownState {
    pub fn view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let mut col = widget::column::with_capacity(3)
            .spacing(spacing.space_s)
            .width(Length::Fill)
            .height(Length::Fill);

        col = col.push(self.header_row());

        if self.events.is_empty() {
            col = col.push(self.empty_state());
        } else {
            let cards: Vec<Element<'_, Message>> = self
                .events
                .iter()
                .map(|e| self.event_card(e, use_12h))
                .collect();
            col = col.push(
                widget::flex_row(cards)
                    .spacing(spacing.space_s as u16)
                    .min_item_width(280.0)
                    .width(Length::Fill),
            );
        }

        col.into()
    }

    fn header_row(&self) -> Element<'_, Message> {
        let mut header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("countdown-title")).width(Length::Fill));

        if !self.events.is_empty() {
            let (icon, tooltip) = if self.edit_mode {
                ("object-select-symbolic", fl!("tooltip-done-editing"))
            } else {
                ("edit-symbolic", fl!("tooltip-edit-mode"))
            };
            header = header.push(
                widget::button::icon(widget::icon::from_name(icon))
                    .tooltip(tooltip)
                    .on_press(Message::ToggleEditMode),
            );
        }

        header
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .tooltip(fl!("tooltip-add"))
                    .on_press(Message::OpenSettings),
            )
            .into()
    }

    fn empty_state(&self) -> Element<'_, Message> {
        let icon = widget::icon::icon(crate::app::bundled_icon(crate::app::COUNTDOWN_ICON))
            .size(128)
            .class(cosmic::theme::Svg::Custom(std::rc::Rc::new(
                |theme: &cosmic::Theme| cosmic::iced_widget::svg::Style {
                    color: Some(theme.cosmic().palette.neutral_5.into()),
                },
            )));

        let empty = widget::column::with_capacity(2)
            .spacing(16)
            .align_x(Alignment::Center)
            .push(icon)
            .push(
                widget::button::suggested(fl!("create-countdown"))
                    .on_press(Message::OpenSettings),
            );

        widget::container(empty)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn event_card<'a>(&'a self, event: &'a CountdownEvent, use_12h: bool) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();
        let id = event.id;
        let now = Local::now();
        let secs = event.seconds_until(now);
        let passed = secs <= 0;
        let (d, h, m, s) = dhms(secs);

        let big = widget::text(if d > 0 {
            fl!(
                "countdown-dhms",
                days = d.to_string(),
                hours = h.to_string(),
                minutes = m.to_string(),
                seconds = s.to_string()
            )
        } else {
            format!("{h:02}:{m:02}:{s:02}")
        })
        .size(34.0)
        .font(light_font());

        // A passed one-off counts up, shown in a muted tone so it reads as done
        // rather than as an upcoming event.
        let cosmic_theme = cosmic::theme::active();
        let big = if passed {
            big.class(cosmic::theme::Text::Color(
                cosmic_theme.cosmic().palette.neutral_6.into(),
            ))
        } else {
            big
        };

        let when = format_target(event.target, use_12h);
        let sub = if passed {
            fl!("countdown-ago", when = when)
        } else {
            when
        };

        let mut card_col = widget::column::with_capacity(5)
            .spacing(spacing.space_xxs)
            .align_x(Alignment::Center)
            .padding(spacing.space_s)
            .push(widget::text::title4(&event.label))
            .push(big)
            .push(widget::text::caption(sub));

        let mut badges = widget::row::with_capacity(2).spacing(spacing.space_xxs);
        if event.yearly {
            badges = badges.push(widget::text::caption(fl!("countdown-yearly")));
        }
        if !event.reminders.is_empty() {
            badges = badges.push(widget::text::caption(fl!(
                "countdown-reminder-count",
                count = event.reminders.len().to_string()
            )));
        }
        card_col = card_col.push(badges);

        if self.edit_mode {
            card_col = card_col.push(
                widget::button::destructive(fl!("delete")).on_press(Message::Delete(id)),
            );
        }

        let card = widget::container(card_col)
            .width(Length::Fill)
            .max_width(340.0)
            .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                let cosmic = theme.cosmic();
                let mut style = cosmic::iced_widget::container::Catalog::style(
                    theme,
                    &cosmic::theme::Container::Primary,
                );
                style.border.radius = cosmic.radius_s().into();
                style.background = Some(Color::from(cosmic.bg_component_color()).into());
                style
            })));

        // Pressing a card opens its editor, matching Alarm. Suppressed in edit
        // mode so the press target doesn't fight the delete button.
        if self.edit_mode {
            card.into()
        } else {
            widget::mouse_area(card)
                .on_press(Message::StartEditEvent(id))
                .into()
        }
    }

    /// Create/edit form for the context drawer.
    pub fn settings_view(&self, use_12h: bool) -> Element<'_, Message> {
        let spacing = 12;
        let editing = self.editing_id.is_some();
        let mut col = widget::column::with_capacity(14).spacing(spacing);

        col = col.push(
            widget::text_input(fl!("countdown-label-placeholder"), &self.edit_label)
                .id(widget::Id::new("countdown-label-input"))
                .on_input(Message::EditLabel),
        );

        // Date. The calendar is tall enough to dominate the drawer, so it lives
        // in a popup behind a button showing the current selection.
        //
        // The model is owned by the page state because `calendar` borrows it for
        // the lifetime of the element. The callback stays `'static` by passing
        // plain y/m/d through and validating in the update handler.
        let date_label = self
            .edit_date()
            .map(|d| d.format("%a %d %b %Y").to_string())
            .unwrap_or_else(|| fl!("countdown-pick-date"));

        // Bundled glyph rather than a theme name: the nav icon is already this
        // calendar, and unproven `from_name` lookups have silently rendered
        // nothing twice in this app.
        // No width set: the chip shrink-wraps so the time stepper can sit beside
        // it. `.width(Length::Fill)` would be ignored anyway — libcosmic's
        // text-button builder applies the requested width to its inner row and
        // then wraps that in an outer button left at `Shrink`.
        let date_button = widget::button::standard(date_label)
            .leading_icon(crate::app::bundled_icon(crate::app::COUNTDOWN_ICON))
            .on_press(Message::ToggleCalendar);

        let mut date_field = widget::popover(date_button)
            .position(widget::popover::Position::Bottom)
            // Modal so a click outside lands on the popup's backdrop and closes
            // it, rather than falling through to the form underneath.
            .modal(true)
            .on_close(Message::ToggleCalendar);

        if self.show_calendar {
            let calendar = widget::calendar(
                &self.edit_calendar,
                |date| {
                    Message::EditDate(
                        i32::from(date.year()),
                        u32::from(date.month() as u8),
                        u32::from(date.day() as u8),
                    )
                },
                || Message::ShowPrevMonth,
                || Message::ShowNextMonth,
                jiff::civil::Weekday::Monday,
            );
            // The calendar draws no background of its own, so give the popup one
            // or it renders over the form as floating text.
            date_field = date_field.popup(
                widget::container(calendar)
                    .padding(spacing)
                    .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                        let cosmic = theme.cosmic();
                        let mut style = cosmic::iced_widget::container::Catalog::style(
                            theme,
                            &cosmic::theme::Container::Primary,
                        );
                        style.border.radius = cosmic.radius_s().into();
                        style.background =
                            Some(Color::from(cosmic.bg_component_color()).into());
                        style
                    }))),
            );
        }

        // Time, reusing the alarm page's vertical stepper.
        let (hour_display, period): (u32, Option<String>) = if use_12h {
            let (h, is_pm) = crate::time_format::to_12h(self.edit_hour);
            (h, Some(if is_pm { fl!("pm") } else { fl!("am") }))
        } else {
            (self.edit_hour, None)
        };

        // Date and time share one line, aligned to the left edge of the form
        // like every other field. `align_y` is about the cross axis: the
        // stepper's centre line is its digits, so this puts the date chip level
        // with them rather than with the increment buttons. `time_picker_row`
        // rather than `time_picker` because the latter's full-width wrapper
        // would centre the digits inside the row's leftover space.
        let when_row = widget::row::with_capacity(3)
            .spacing(spacing)
            .align_y(Alignment::Center)
            .push(date_field)
            .push(time_picker_row(vec![
                TimeUnit::new(
                    format!("{hour_display:02}"),
                    Message::EditHour((self.edit_hour + 1) % 24),
                    Message::EditHour((self.edit_hour + 23) % 24),
                ),
                TimeUnit::new(
                    format!("{:02}", self.edit_minute),
                    Message::EditMinute((self.edit_minute + 1) % 60),
                    Message::EditMinute((self.edit_minute + 59) % 60),
                ),
            ]))
            .push_maybe(period.map(|p| widget::text::body(p)));

        col = col.push(when_row);

        col = col.push(widget::divider::horizontal::default());

        col = col.push(
            widget::checkbox(self.edit_yearly)
                .label(fl!("countdown-yearly-label"))
                .on_toggle(|_| Message::ToggleYearly),
        );

        // Reminders: a fixed preset list, each a toggle.
        col = col.push(widget::text::body(fl!("countdown-reminders")));
        for reminder in Reminder::ALL {
            let checked = self.edit_reminders.contains(&reminder);
            col = col.push(
                widget::checkbox(checked)
                    .label(reminder.display_name())
                    .on_toggle(move |_| Message::ToggleReminder(reminder)),
            );
        }

        col = col.push(widget::divider::horizontal::default());
        col = col.push(sound_selector_view(
            fl!("sound"),
            &self.edit_sound,
            Message::EditSound,
            Message::BrowseCustomSound,
        ));

        col = col.push(widget::divider::horizontal::default());
        let actions = if editing {
            widget::row::with_capacity(3)
                .spacing(8)
                .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEdit))
                .push(widget::button::suggested(fl!("save")).on_press(Message::SaveEditEvent))
                .push(
                    widget::button::destructive(fl!("delete")).on_press_maybe(
                        self.editing_id.map(Message::Delete),
                    ),
                )
        } else {
            widget::row::with_capacity(1)
                .push(widget::button::suggested(fl!("countdown-add")).on_press(Message::AddEvent))
        };

        col.push(actions).into()
    }
}

/// "Sat 14 Mar 2026, 09:00" — date plus time in the user's clock format.
fn format_target(target: chrono::DateTime<Local>, use_12h: bool) -> String {
    let date = target.format("%a %d %b %Y").to_string();
    let time = crate::time_format::format_time_of_day(&target, use_12h);
    format!("{date}, {time}")
}
