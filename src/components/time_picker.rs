// SPDX-License-Identifier: MIT
//
// Shared vertical-stepper time picker.
//
// Each unit is rendered as a centered column: an up button on top, a large
// light-weight value in the middle, and a down button on the bottom. Units are
// joined by a ":" separator. The vertical layout is much narrower than a
// horizontal `- value +` row, so it stays fully on-screen on narrow windows.

use cosmic::iced::font::Weight;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;

/// Font weight 300 (Light) for the value display, matching the timer/stopwatch pages.
fn light_font() -> cosmic::iced::Font {
    cosmic::iced::Font {
        weight: Weight::Light,
        ..cosmic::font::default()
    }
}

/// A single unit (hours, minutes, or seconds) in a [`time_picker`].
pub struct TimeUnit<M> {
    pub value: String,
    pub on_increment: M,
    pub on_decrement: M,
}

impl<M> TimeUnit<M> {
    pub fn new(value: impl Into<String>, on_increment: M, on_decrement: M) -> Self {
        Self {
            value: value.into(),
            on_increment,
            on_decrement,
        }
    }
}

/// Build a compact vertical-stepper time picker from the given units.
pub fn time_picker<M: Clone + 'static>(units: Vec<TimeUnit<M>>) -> Element<'static, M> {
    let spacing = cosmic::theme::spacing();

    let mut row = widget::row::with_capacity(units.len() * 2)
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center);

    let last = units.len().saturating_sub(1);
    for (i, unit) in units.into_iter().enumerate() {
        let column = widget::column::with_capacity(3)
            .spacing(spacing.space_xxs)
            .align_x(Alignment::Center)
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .on_press(unit.on_increment),
            )
            .push(
                widget::container(widget::text(unit.value).size(28.0).font(light_font()))
                    .align_x(Alignment::Center)
                    .width(Length::Shrink),
            )
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                    .on_press(unit.on_decrement),
            );

        row = row.push(column);

        if i != last {
            row = row.push(widget::text(":").size(28.0).font(light_font()));
        }
    }

    widget::container(row)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .into()
}
