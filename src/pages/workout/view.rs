// SPDX-License-Identifier: MIT
//
// Workout view: stats-free card grid mirroring the Timer page, plus the
// create/edit settings sidebar.

use super::Message;
use super::model::*;
use crate::components::reorder_list::ReorderList;
use crate::components::{CircularProgress, sound_selector_view};
use crate::fl;
use cosmic::iced::font::Weight;
use cosmic::iced::{Alignment, Color, Length};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::Duration;

fn light_font() -> cosmic::iced::Font {
    cosmic::iced::Font {
        weight: Weight::Light,
        ..cosmic::font::default()
    }
}

/// Format a short interval as MM:SS.
fn mmss(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

impl WorkoutState {
    pub fn view(&self) -> Element<'_, Message> {
        if self.edit_mode {
            self.edit_mode_view()
        } else {
            self.card_grid_view()
        }
    }

    fn card_grid_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::column::with_capacity(2)
            .spacing(spacing.space_s)
            .width(Length::Fill)
            .height(Length::Fill);

        col = col.push(self.header_row());

        if self.workouts.is_empty() {
            col = col.push(self.empty_state());
        } else {
            let cards: Vec<Element<'_, Message>> = self
                .workouts
                .iter()
                .map(|w| self.workout_card(w))
                .collect();
            let grid = widget::flex_row(cards)
                .spacing(spacing.space_s as u16)
                .min_item_width(280.0)
                .width(Length::Fill);
            col = col.push(grid);
        }

        col.into()
    }

    fn workout_card<'a>(&'a self, w: &'a WorkoutEntry) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();
        let id = w.id;

        let total = w.phase_total().as_secs_f32();
        let progress = if total > 0.0 {
            1.0 - (w.remaining.as_secs_f32() / total)
        } else {
            0.0
        };

        // Work phases use the accent color; rest/prep use a neutral tone.
        let cosmic = cosmic::theme::active();
        let fill_color: Color = if w.phase == Phase::Work {
            cosmic.cosmic().accent_color().into()
        } else {
            cosmic.cosmic().palette.neutral_6.into()
        };
        let track_color = Color::from_rgba(0.5, 0.5, 0.5, 0.15);

        let circle_size = 170.0;
        let circle = CircularProgress::new(progress)
            .size(circle_size)
            .stroke_width(6.0)
            .track_color(track_color)
            .fill_color(fill_color)
            .view();

        let time_text = widget::text(mmss(w.remaining)).size(28.0).font(light_font());

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

        let label = widget::container(widget::text::title4(&w.label))
            .align_x(Alignment::Center)
            .width(Length::Fill);

        let status = widget::container(widget::text::caption(fl!(
            "workout-status",
            phase = w.phase.display_name(),
            round = w.current_round.to_string(),
            rounds = w.rounds.to_string(),
            set = w.current_set.to_string(),
            sets = w.sets.to_string()
        )))
        .align_x(Alignment::Center)
        .width(Length::Fill);

        let controls = self.card_controls(w);

        let card_col = widget::column::with_capacity(4)
            .spacing(spacing.space_xxs)
            .align_x(Alignment::Center)
            .padding(spacing.space_s)
            .push(circle_with_time)
            .push(label)
            .push(status)
            .push(controls);

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

        if !w.is_running {
            widget::mouse_area(card)
                .on_press(Message::StartEditWorkout(id))
                .into()
        } else {
            card.into()
        }
    }

    fn card_controls(&self, w: &WorkoutEntry) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let id = w.id;

        let (primary_icon, primary_tooltip, primary_msg, use_accent) = if w.is_running {
            (
                "media-playback-pause-symbolic",
                fl!("tooltip-pause"),
                Message::Pause(id),
                false,
            )
        } else if w.has_started() && w.phase != Phase::Done {
            (
                "media-playback-start-symbolic",
                fl!("tooltip-resume"),
                Message::Resume(id),
                true,
            )
        } else {
            (
                "media-playback-start-symbolic",
                fl!("tooltip-start"),
                Message::Start(id),
                true,
            )
        };

        let primary_btn = widget::tooltip(
            widget::button::custom(
                widget::container(widget::icon::from_name(primary_icon).size(24).icon())
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

        let small_btn = |icon: &str, tip: String, msg: Message| -> Element<'_, Message> {
            widget::tooltip(
                widget::button::custom(
                    widget::container(widget::icon::from_name(icon).size(16).icon())
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .width(32)
                        .height(32),
                )
                .class(cosmic::theme::Button::Standard)
                .on_press(msg),
                widget::text::body(tip),
                widget::tooltip::Position::Top,
            )
            .into()
        };

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

        let left_slot: Element<'_, Message> = if w.has_started() {
            small_btn("edit-undo-symbolic", fl!("tooltip-reset"), Message::Reset(id))
        } else {
            invisible_btn()
        };
        let right_slot: Element<'_, Message> = if w.started && w.phase != Phase::Done {
            small_btn(
                "media-skip-forward-symbolic",
                fl!("tooltip-skip"),
                Message::Skip(id),
            )
        } else {
            invisible_btn()
        };

        widget::container(
            widget::row::with_capacity(3)
                .spacing(spacing.space_s)
                .align_y(Alignment::Center)
                .push(left_slot)
                .push(primary_btn)
                .push(right_slot),
        )
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .into()
    }

    fn edit_mode_view(&self) -> Element<'_, Message> {
        let cosmic::cosmic_theme::Spacing {
            space_xxs,
            space_xs,
            space_xxxs,
            ..
        } = cosmic::theme::spacing();

        let mut col = widget::column::with_capacity(self.workouts.len() + 2).spacing(space_xxs);
        col = col.push(self.header_row());

        if self.workouts.is_empty() {
            col = col.push(self.empty_state());
            return col.into();
        }

        let dragging = self.dragging_index;
        let card_rows: Vec<Element<'_, Message>> = self
            .workouts
            .iter()
            .enumerate()
            .map(|(i, w)| {
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

                let id = w.id;
                let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(3);
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
                let info_col = widget::column::with_capacity(2)
                    .spacing(space_xxxs)
                    .push(widget::text::body(&w.label))
                    .push(widget::text::caption(fl!(
                        "workout-summary",
                        work = w.work_secs.to_string(),
                        rest = w.rest_secs.to_string(),
                        rounds = w.rounds.to_string()
                    )));
                items.push(info_col.width(Length::Fill).into());
                items.push(
                    widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                        .extra_small()
                        .tooltip(fl!("tooltip-delete"))
                        .on_press(Message::Delete(id))
                        .into(),
                );

                let content = widget::row::with_children(items)
                    .spacing(space_xs)
                    .align_y(Alignment::Center);

                widget::container(content)
                    .padding(8)
                    .width(Length::Fill)
                    .class(cosmic::theme::Container::Custom(Box::new(|theme| {
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
        let item_count = self.workouts.len();
        let snapshot: Vec<String> = self.workouts.iter().map(|w| w.label.clone()).collect();

        let reorder_list = ReorderList::new(cards, item_count, self.dragging_index)
            .on_start_drag(Message::StartDrag)
            .on_reorder(Message::Reorder)
            .on_finish(Message::FinishDrag)
            .on_cancel(Message::CancelDrag)
            .drag_icon(move |index, offset| {
                let label = snapshot
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| "Workout".to_string());
                let content = widget::row::with_children(vec![
                    widget::icon::from_name("grip-lines-symbolic")
                        .size(16)
                        .icon()
                        .into(),
                    widget::text::body(label).width(Length::Fill).into(),
                ])
                .spacing(space_xs)
                .align_y(Alignment::Center);

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
        col.into()
    }

    fn header_row(&self) -> Element<'_, Message> {
        let mut header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("workout-title")).width(Length::Fill));

        if !self.workouts.is_empty() {
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
                .on_press(Message::OpenSettings),
        );

        header.into()
    }

    fn empty_state(&self) -> Element<'_, Message> {
        let icon = widget::icon::from_name("preferences-system-time-symbolic")
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
            .push(widget::button::suggested(fl!("create-workout")).on_press(Message::OpenSettings));

        widget::container(empty_state)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Create/edit sidebar.
    pub fn settings_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let editing = self.editing_id.is_some();
        let mut col = widget::column::with_capacity(16).spacing(spacing);

        col = col.push(widget::text::title4(if editing {
            fl!("workout-edit")
        } else {
            fl!("workout-new")
        }));

        col = col.push(
            widget::text_input(fl!("workout-label-placeholder"), &self.edit_label)
                .id(widget::Id::new("workout-label-input"))
                .on_input(Message::EditLabel),
        );

        // Presets
        col = col.push(widget::text::body(fl!("workout-presets")));
        let presets = widget::row::with_capacity(2)
            .spacing(8)
            .push(
                widget::button::standard(fl!("workout-preset-tabata"))
                    .on_press(Message::ApplyPreset(Preset::Tabata)),
            )
            .push(
                widget::button::standard(fl!("workout-preset-hiit"))
                    .on_press(Message::ApplyPreset(Preset::Hiit)),
            );
        col = col.push(presets);

        col = col.push(widget::divider::horizontal::default());

        col = col.push(stepper_row(
            fl!("workout-prep-label"),
            fl!("seconds-value", value = self.edit_prep.to_string()),
            Message::EditPrep(self.edit_prep.saturating_sub(5)),
            Message::EditPrep(self.edit_prep + 5),
        ));
        col = col.push(stepper_row(
            fl!("workout-work-label"),
            fl!("seconds-value", value = self.edit_work.to_string()),
            Message::EditWork(self.edit_work.saturating_sub(5)),
            Message::EditWork(self.edit_work + 5),
        ));
        col = col.push(stepper_row(
            fl!("workout-rest-label"),
            fl!("seconds-value", value = self.edit_rest.to_string()),
            Message::EditRest(self.edit_rest.saturating_sub(5)),
            Message::EditRest(self.edit_rest + 5),
        ));
        col = col.push(stepper_row(
            fl!("workout-rounds-label"),
            self.edit_rounds.to_string(),
            Message::EditRounds(self.edit_rounds.saturating_sub(1)),
            Message::EditRounds(self.edit_rounds + 1),
        ));
        col = col.push(stepper_row(
            fl!("workout-sets-label"),
            self.edit_sets.to_string(),
            Message::EditSets(self.edit_sets.saturating_sub(1)),
            Message::EditSets(self.edit_sets + 1),
        ));
        col = col.push(stepper_row(
            fl!("workout-set-rest-label"),
            fl!("seconds-value", value = self.edit_set_rest.to_string()),
            Message::EditSetRest(self.edit_set_rest.saturating_sub(10)),
            Message::EditSetRest(self.edit_set_rest + 10),
        ));

        col = col.push(widget::divider::horizontal::default());
        col = col.push(sound_selector_view(
            fl!("sound"),
            &self.edit_sound,
            Message::EditSound,
            Message::BrowseCustomSound,
        ));

        col = col.push(widget::divider::horizontal::default());
        if editing {
            let actions = widget::row::with_capacity(2)
                .spacing(8)
                .push(
                    widget::button::standard(fl!("cancel"))
                        .on_press(Message::CancelEditWorkout),
                )
                .push(
                    widget::button::suggested(fl!("save")).on_press(Message::SaveEditWorkout),
                );
            col = col.push(actions);
        } else {
            col = col
                .push(widget::button::suggested(fl!("workout-add")).on_press(Message::AddWorkout));
        }

        col.into()
    }
}

/// A label + value + decrement/increment stepper row for the settings form.
fn stepper_row(
    label: String,
    value: String,
    on_decrement: Message,
    on_increment: Message,
) -> Element<'static, Message> {
    widget::row::with_capacity(4)
        .spacing(8)
        .align_y(Alignment::Center)
        .push(widget::text::body(label).width(Length::Fixed(110.0)))
        .push(
            widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                .on_press(on_decrement),
        )
        .push(widget::text::body(value))
        .push(
            widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                .on_press(on_increment),
        )
        .into()
}
