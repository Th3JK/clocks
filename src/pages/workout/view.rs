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
        // The block editor takes over the page while open — it needs the room.
        if let Some(id) = self.editing_blocks_id
            && let Some(w) = self.workouts.iter().find(|w| w.id == id)
        {
            return self.block_editor_view(&w.label);
        }
        // Focus mode wins over every other mode. The id is re-looked-up rather
        // than trusted: if it no longer resolves (deleted elsewhere) we fall back
        // to the list instead of rendering a blank page.
        if let Some(id) = self.focused_id
            && let Some(w) = self.workouts.iter().find(|w| w.id == id)
        {
            return self.focus_view(w);
        }
        if self.edit_mode {
            self.edit_mode_view()
        } else {
            self.card_grid_view()
        }
    }

    /// Full-page block editor: blocks as cards, steps as rows inside them.
    /// Operates on `edit_blocks`, a working copy, so closing without saving
    /// discards the changes.
    fn block_editor_view(&self, label: &str) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .spacing(spacing.space_xs)
            .push(
                widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
                    .on_press(Message::CloseBlockEditor),
            )
            .push(
                widget::container(widget::text::title3(fl!(
                    "workout-edit-blocks-title",
                    label = label.to_string()
                )))
                .width(Length::Fill),
            )
            .push(widget::button::suggested(fl!("save")).on_press(Message::SaveBlocks));

        let mut col = widget::column::with_capacity(self.edit_blocks.len() + 3)
            .spacing(spacing.space_s)
            .width(Length::Fill);

        for (bi, block) in self.edit_blocks.iter().enumerate() {
            col = col.push(self.block_card(bi, block));
        }

        // Total of the working copy, so the effect of an edit is visible before
        // saving. Uses the same flattening the runtime will use.
        let total: u32 = flatten(&self.edit_blocks).iter().map(|s| s.secs).sum();
        let footer = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(
                widget::button::standard(fl!("workout-add-block"))
                    .leading_icon(widget::icon::from_name("list-add-symbolic"))
                    .on_press(Message::AddBlock),
            )
            .push(
                widget::container(widget::text::body(fl!(
                    "workout-total-duration",
                    total = mmss(Duration::from_secs(total as u64))
                )))
                .align_x(Alignment::End)
                .width(Length::Fill),
            );
        col = col.push(footer);

        widget::column::with_capacity(2)
            .spacing(spacing.space_s)
            .padding(spacing.space_xs)
            .push(header)
            .push(widget::scrollable(col).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// One block: repeat count, skip toggle, reorder/remove, and its steps.
    fn block_card<'a>(&'a self, bi: usize, block: &'a Block) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();

        let icon_btn = |name: &str, msg: Message| {
            widget::button::icon(widget::icon::from_name(name))
                .extra_small()
                .on_press(msg)
        };

        let head = widget::row::with_capacity(6)
            .align_y(Alignment::Center)
            .spacing(spacing.space_xxs)
            .push(
                widget::text::body(fl!("workout-block-n", n = (bi + 1).to_string()))
                    .width(Length::Fill),
            )
            .push(widget::text::caption(fl!("workout-repeat")))
            .push(icon_btn(
                "list-remove-symbolic",
                Message::SetBlockRepeat(bi, block.repeat.saturating_sub(1)),
            ))
            .push(widget::text::body(format!("{}×", block.repeat)))
            .push(icon_btn(
                "list-add-symbolic",
                Message::SetBlockRepeat(bi, block.repeat + 1),
            ))
            .push(icon_btn("go-up-symbolic", Message::MoveBlock(bi, -1)))
            .push(icon_btn("go-down-symbolic", Message::MoveBlock(bi, 1)))
            .push(icon_btn("edit-delete-symbolic", Message::RemoveBlock(bi)));

        let mut col = widget::column::with_capacity(block.steps.len() + 3)
            .spacing(spacing.space_xxs)
            .push(head)
            .push(widget::divider::horizontal::default());

        for (si, step) in block.steps.iter().enumerate() {
            col = col.push(self.step_row(bi, si, step));
        }

        let footer = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .spacing(spacing.space_xs)
            .push(
                widget::button::standard(fl!("workout-add-step"))
                    .leading_icon(widget::icon::from_name("list-add-symbolic"))
                    .on_press(Message::AddStep(bi)),
            )
            .push(
                widget::container(
                    widget::checkbox(block.skip_last_recovery)
                        .label(fl!("workout-skip-last-recovery"))
                        .on_toggle(move |_| Message::ToggleSkipLastRecovery(bi)),
                )
                .align_x(Alignment::End)
                .width(Length::Fill),
            );
        col = col.push(footer);

        widget::container(col)
            .padding(spacing.space_s)
            .width(Length::Fill)
            .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                let cosmic = theme.cosmic();
                let mut style = cosmic::iced_widget::container::Catalog::style(
                    theme,
                    &cosmic::theme::Container::Primary,
                );
                style.border.radius = cosmic.radius_s().into();
                style.background = Some(Color::from(cosmic.bg_component_color()).into());
                style
            })))
            .into()
    }

    /// One step row: label, duration, kind picker, reorder/remove.
    fn step_row<'a>(&'a self, bi: usize, si: usize, step: &'a Step) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();

        let icon_btn = |name: &str, msg: Message| {
            widget::button::icon(widget::icon::from_name(name))
                .extra_small()
                .on_press(msg)
        };

        // Kind is an explicit three-way choice rather than inferred from the
        // label — label matching would break in five of the six locales.
        let kind_picker = widget::row::with_children(
            StepKind::ALL
                .iter()
                .map(|k| {
                    let k = *k;
                    let selected = step.kind == k;
                    let btn = widget::button::text(k.display_name()).class(if selected {
                        cosmic::theme::Button::Suggested
                    } else {
                        cosmic::theme::Button::Standard
                    });
                    btn.on_press(Message::SetStepKind(bi, si, k)).into()
                })
                .collect::<Vec<_>>(),
        )
        .spacing(spacing.space_xxxs);

        widget::row::with_capacity(6)
            .align_y(Alignment::Center)
            .spacing(spacing.space_xxs)
            .push(
                widget::text_input(step.kind.default_label(), &step.label)
                    .on_input(move |v| Message::SetStepLabel(bi, si, v))
                    .width(Length::Fill),
            )
            .push(icon_btn(
                "list-remove-symbolic",
                Message::SetStepSecs(bi, si, step.secs.saturating_sub(5)),
            ))
            .push(widget::text::body(fl!(
                "seconds-value",
                value = step.secs.to_string()
            )))
            .push(icon_btn(
                "list-add-symbolic",
                Message::SetStepSecs(bi, si, step.secs + 5),
            ))
            .push(kind_picker)
            .push(icon_btn("go-up-symbolic", Message::MoveStep(bi, si, -1)))
            .push(icon_btn("go-down-symbolic", Message::MoveStep(bi, si, 1)))
            .push(icon_btn("edit-delete-symbolic", Message::RemoveStep(bi, si)))
            .into()
    }

    /// Full-page view of a single workout: back header, large ring, status, controls.
    fn focus_view<'a>(&'a self, w: &'a WorkoutEntry) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();

        let back = widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
            .on_press(Message::Unfocus);

        let header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(back)
            .push(
                widget::container(widget::text::title3(&w.label))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
            // Balances the back button so the title stays optically centred.
            .push(widget::Space::new().width(40.0));

        let total = w.step_total().as_secs_f32();
        let progress = if total > 0.0 {
            1.0 - (w.remaining.as_secs_f32() / total)
        } else {
            0.0
        };

        let cosmic = cosmic::theme::active();
        let fill_color: Color = if w.is_effort() {
            cosmic.cosmic().accent_color().into()
        } else {
            cosmic.cosmic().palette.neutral_6.into()
        };

        let circle_size = 280.0;
        let circle = CircularProgress::new(progress)
            .size(circle_size)
            .stroke_width(10.0)
            .track_color(Color::from_rgba(0.5, 0.5, 0.5, 0.15))
            .fill_color(fill_color)
            .view();

        let time_text = widget::text(mmss(w.remaining)).size(56.0).font(light_font());

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

        let status = widget::container(widget::text::body(fl!(
            "workout-status-blocks",
            step = w.current_step().map(|s| s.label.clone()).unwrap_or_else(|| fl!("workout-done")),
            block = w.current_step().map(|s| s.block.to_string()).unwrap_or_default(),
            blocks = w.current_step().map(|s| s.blocks.to_string()).unwrap_or_default(),
            rep = w.current_step().map(|s| s.rep.to_string()).unwrap_or_default(),
            reps = w.current_step().map(|s| s.reps.to_string()).unwrap_or_default()
        )))
        .align_x(Alignment::Center)
        .width(Length::Fill);

        widget::column::with_capacity(4)
            .spacing(spacing.space_s)
            .padding(spacing.space_xs)
            .push(header)
            .push(hero)
            .push(status)
            .push(self.card_controls(w))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
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

        let total = w.step_total().as_secs_f32();
        let progress = if total > 0.0 {
            1.0 - (w.remaining.as_secs_f32() / total)
        } else {
            0.0
        };

        // Effort steps use the accent color; rest/prep use a neutral tone.
        let cosmic = cosmic::theme::active();
        let fill_color: Color = if w.is_effort() {
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
            "workout-status-blocks",
            step = w.current_step().map(|s| s.label.clone()).unwrap_or_else(|| fl!("workout-done")),
            block = w.current_step().map(|s| s.block.to_string()).unwrap_or_default(),
            blocks = w.current_step().map(|s| s.blocks.to_string()).unwrap_or_default(),
            rep = w.current_step().map(|s| s.rep.to_string()).unwrap_or_default(),
            reps = w.current_step().map(|s| s.reps.to_string()).unwrap_or_default()
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

        // Pressing the card enters focus mode, mirroring World Clocks' detail
        // view. Editing moved to the pencil in the controls row.
        widget::mouse_area(card).on_press(Message::Focus(id)).into()
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
        } else if w.has_started() && !w.is_finished() {
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

        // Takes a built `Icon` rather than a theme name so callers can pass a
        // bundled SVG for glyphs the icon theme may not have.
        let small_btn = |icon: widget::icon::Icon, tip: String, msg: Message| -> Element<'_, Message> {
            widget::tooltip(
                widget::button::custom(
                    widget::container(icon.size(16))
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

        // Reset takes priority once the workout has started. Before that the slot
        // is free, so it carries the edit affordance that pressing the card used
        // to provide (the press now enters focus mode). A started workout is still
        // editable after a reset, or from edit mode.
        let left_slot: Element<'_, Message> = if w.has_started() {
            small_btn(
                widget::icon::from_name("edit-undo-symbolic").icon(),
                fl!("tooltip-reset"),
                Message::Reset(id),
            )
        } else {
            small_btn(
                widget::icon::from_name("edit-symbolic").icon(),
                fl!("workout-edit"),
                Message::StartEditWorkout(id),
            )
        };
        let right_slot: Element<'_, Message> = if w.started && !w.is_finished() {
            small_btn(
                widget::icon::from_name("media-skip-forward-symbolic").icon(),
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
                        blocks = w.blocks.len().to_string(),
                        total = mmss(w.total_duration())
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

        col = col.push(
            widget::text_input(fl!("workout-label-placeholder"), &self.edit_label)
                .id(widget::Id::new("workout-label-input"))
                .on_input(Message::EditLabel),
        );

        // Editing an existing workout: structure lives in the block editor. The
        // steppers below describe a simple prep/work/rest layout and cannot
        // represent an arbitrary block list, so showing them here would either
        // lie about the current structure or silently overwrite it on save.
        if editing {
            if let Some(id) = self.editing_id {
                col = col.push(widget::divider::horizontal::default());
                col = col.push(
                    widget::button::standard(fl!("workout-edit-blocks"))
                        .leading_icon(widget::icon::from_name("view-list-symbolic"))
                        .on_press(Message::OpenBlockEditor(id)),
                );
                if let Some(w) = self.workouts.iter().find(|w| w.id == id) {
                    col = col.push(widget::text::caption(fl!(
                        "workout-summary",
                        blocks = w.blocks.len().to_string(),
                        total = mmss(w.total_duration())
                    )));
                }
            }
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
                .push(
                    widget::button::standard(fl!("cancel"))
                        .on_press(Message::CancelEditWorkout),
                )
                .push(widget::button::suggested(fl!("save")).on_press(Message::SaveEditWorkout));
            return col.push(actions).into();
        }

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
