// SPDX-License-Identifier: MIT
//
// Pomodoro view functions: stats header, card grid, edit mode, empty state,
// and the settings sidebar. Mirrors the Timer page design language.

use super::Message;
use super::model::*;
use crate::components::reorder_list::ReorderList;
use crate::components::{CircularProgress, format_duration_hms, sound_selector_view};
use crate::fl;
use cosmic::iced::font::Weight;
use cosmic::iced::{Alignment, Color, Length};
use cosmic::prelude::*;
use cosmic::widget;

/// Font weight 300 (Light) for the pomodoro time display, matching the other pages.
fn light_font() -> cosmic::iced::Font {
    cosmic::iced::Font {
        weight: Weight::Light,
        ..cosmic::font::default()
    }
}

/// Format a focus duration as "Xh Ym" (or "Ym" under an hour).
fn format_focus(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        fl!(
            "focus-hours-minutes",
            hours = h.to_string(),
            minutes = m.to_string()
        )
    } else {
        fl!("focus-minutes", minutes = m.to_string())
    }
}

impl PomodoroState {
    /// Main view: dispatches to focus mode, card grid, or edit mode.
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

    /// Full-page view of a single pomodoro: back header, large ring, session info.
    fn focus_view<'a>(&'a self, timer: &'a PomodoroTimer) -> Element<'a, Message> {
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

        let total = timer.session_total().as_secs_f32();
        let progress = if total > 0.0 {
            1.0 - (timer.remaining.as_secs_f32() / total)
        } else {
            0.0
        };

        // Work sessions use the accent colour, breaks a neutral tone — the same
        // convention the workout page uses to distinguish effort from recovery.
        let cosmic = cosmic::theme::active();
        let fill_color: Color = if timer.session_type == SessionType::Work {
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

        let time_text = widget::text(format_duration_hms(timer.remaining))
            .size(56.0)
            .font(light_font());

        let hero = widget::container(
            cosmic::iced::widget::stack![
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

        let session = widget::container(widget::text::body(fl!(
            "session-info",
            number = timer.session_number.to_string(),
            session_type = timer.session_type.display_name()
        )))
        .align_x(Alignment::Center)
        .width(Length::Fill);

        let progress_info = widget::container(widget::text::caption(fl!(
            "progress-info",
            completed = timer.completed_work_sessions.to_string(),
            target = timer.target_sessions.to_string(),
            focused = (timer.total_focused_secs / 60).to_string()
        )))
        .align_x(Alignment::Center)
        .width(Length::Fill);

        widget::column::with_capacity(5)
            .spacing(spacing.space_s)
            .padding(spacing.space_xs)
            .push(header)
            .push(hero)
            .push(session)
            .push(progress_info)
            .push(self.card_controls(timer))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Base view: header + stats + card grid (or empty state).
    fn card_grid_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let mut col = widget::column::with_capacity(4)
            .spacing(spacing.space_s)
            .width(Length::Fill)
            .height(Length::Fill);

        col = col.push(self.header_row());

        if self.timers.is_empty() {
            col = col.push(self.empty_state());
        } else {
            col = col.push(self.stats_row());

            let cards: Vec<Element<'_, Message>> = self
                .timers
                .iter()
                .map(|timer| self.pomodoro_card(timer))
                .collect();

            let grid = widget::flex_row(cards)
                .spacing(spacing.space_s as u16)
                .min_item_width(280.0)
                .width(Length::Fill);

            col = col.push(grid);
        }

        col.into()
    }

    /// Common height for the three cards in `stats_row`, derived from the tallest
    /// content (the weekly chart) so all three line up. `flex_row` will not level
    /// them for us — see the note on `themed_card`.
    fn stats_card_height() -> f32 {
        let spacing = cosmic::theme::spacing();
        // Approximate rendered height of a `text::caption` line.
        const CAPTION_HEIGHT: f32 = 16.0;
        f32::from(spacing.space_s) * 2.0      // card padding, top + bottom
            + CAPTION_HEIGHT                  // card title
            + f32::from(spacing.space_xxs)    // title -> body gap
            + BAR_MAX_HEIGHT                  // chart bars
            + f32::from(spacing.space_xxxs)   // bars -> weekday label gap
            + CAPTION_HEIGHT // weekday labels
    }

    /// Row of statistics cards: focus today, current streak, and a weekly bar chart.
    fn stats_row(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let focus_card = self.stat_card(fl!("stat-focus-today"), format_focus(self.focus_today()));
        let streak_card = self.stat_card(
            fl!("stat-streak"),
            fl!("stat-streak-days", count = self.current_streak().to_string()),
        );
        let week_card = self.weekly_card();

        widget::flex_row(vec![focus_card, streak_card, week_card])
            .spacing(spacing.space_s as u16)
            .min_item_width(160.0)
            .width(Length::Fill)
            .into()
    }

    /// A single titled statistic card with a large value.
    fn stat_card(&self, title: String, value: String) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let col = widget::column::with_capacity(2)
            .spacing(spacing.space_xxs)
            .push(widget::text::caption(title))
            .push(widget::text(value).size(24.0).font(light_font()));

        themed_card(
            col.into(),
            spacing.space_s,
            Length::Fixed(Self::stats_card_height()),
        )
    }

    /// Weekly focus trend as a 7-bar mini chart.
    fn weekly_card(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let accent = cosmic::theme::active().cosmic().accent_color();

        let days = self.last_7_days();
        let max = days.iter().map(|(_, s)| *s).max().unwrap_or(0).max(1);

        let bars: Vec<Element<'_, Message>> = days
            .iter()
            .map(|(label, secs)| {
                let frac = *secs as f32 / max as f32;
                let bar_height = (frac * BAR_MAX_HEIGHT).max(2.0);
                let accent: Color = accent.into();

                let bar = widget::container(widget::Space::new().width(Length::Fill))
                    .width(Length::Fill)
                    .height(Length::Fixed(bar_height))
                    .class(cosmic::theme::Container::Custom(Box::new(move |theme| {
                        cosmic::iced::widget::container::Style {
                            background: Some(cosmic::iced::Background::Color(accent)),
                            border: cosmic::iced::Border {
                                radius: theme.cosmic().radius_xs().into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })));

                // Anchor the bar to the bottom of a fixed-height area.
                let bar_area = widget::container(bar)
                    .height(Length::Fixed(BAR_MAX_HEIGHT))
                    .align_y(Alignment::End)
                    .width(Length::Fill);

                widget::column::with_capacity(2)
                    .spacing(spacing.space_xxxs)
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .push(bar_area)
                    .push(widget::text::caption(label.clone()))
                    .into()
            })
            .collect();

        let chart = widget::row::with_children(bars)
            .spacing(spacing.space_xxxs)
            .align_y(Alignment::End);

        let col = widget::column::with_capacity(2)
            .spacing(spacing.space_xxs)
            .push(widget::text::caption(fl!("stat-this-week")))
            .push(chart);

        themed_card(
            col.into(),
            spacing.space_s,
            Length::Fixed(Self::stats_card_height()),
        )
    }

    /// A single pomodoro timer card with circular progress, session info, and controls.
    fn pomodoro_card<'a>(&'a self, timer: &'a PomodoroTimer) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();
        let id = timer.id;

        let total = timer.session_total().as_secs_f32();
        let progress = if total > 0.0 {
            1.0 - (timer.remaining.as_secs_f32() / total)
        } else {
            0.0
        };

        let accent = cosmic::theme::active().cosmic().accent_color();
        let track_color = Color::from_rgba(0.5, 0.5, 0.5, 0.15);

        let circle_size = 170.0;
        let circle = CircularProgress::new(progress)
            .size(circle_size)
            .stroke_width(6.0)
            .track_color(track_color)
            .fill_color(accent.into())
            .view();

        let time_text = widget::text(format_duration_hms(timer.remaining))
            .size(28.0)
            .font(light_font());

        let circle_with_time = widget::container(
            cosmic::iced::widget::stack![
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

        let label = widget::container(widget::text::title4(&timer.label))
            .align_x(Alignment::Center)
            .width(Length::Fill);

        let session_info = widget::container(widget::text::caption(fl!(
            "session-info",
            number = timer.session_number.to_string(),
            session_type = timer.session_type.display_name()
        )))
        .align_x(Alignment::Center)
        .width(Length::Fill);

        let controls = self.card_controls(timer);

        let footer = widget::container(widget::text::caption(fl!(
            "progress-info",
            completed = timer.completed_work_sessions.to_string(),
            target = timer.target_sessions.to_string(),
            focused = (timer.total_focused_secs / 60).to_string()
        )))
        .align_x(Alignment::Center)
        .width(Length::Fill);

        let card_col = widget::column::with_capacity(5)
            .spacing(spacing.space_xxs)
            .align_x(Alignment::Center)
            .padding(spacing.space_s)
            .push(circle_with_time)
            .push(label)
            .push(session_info)
            .push(controls)
            .push(footer);

        let card = widget::container(card_col)
            .width(Length::Fill)
            .max_width(340.0)
            .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                let mut style = cosmic::iced::widget::container::Catalog::style(
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

    /// Reset (left) + Play/Pause (center) + Skip (right). Mirrors the Timer card
    /// controls (48px primary, 32px secondary).
    fn card_controls(&self, timer: &PomodoroTimer) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let id = timer.id;

        let (primary_icon, primary_tooltip, primary_msg, use_accent) = if timer.is_running {
            (
                "media-playback-pause-symbolic",
                fl!("tooltip-pause"),
                Message::Pause(id),
                false,
            )
        } else if timer.remaining < timer.session_total() {
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

        // Reset takes priority once started; before that the slot carries the edit
        // affordance that pressing the card used to provide.
        let left_slot: Element<'_, Message> = if timer.has_started() {
            small_btn(
                widget::icon::from_name("edit-undo-symbolic").icon(),
                fl!("tooltip-reset"),
                Message::Reset(id),
            )
        } else {
            small_btn(
                widget::icon::from_name("edit-symbolic").icon(),
                fl!("edit-pomodoro"),
                Message::StartEditPomodoro(id),
            )
        };
        let skip_slot: Element<'_, Message> = small_btn(
            widget::icon::from_name("media-skip-forward-symbolic").icon(),
            fl!("tooltip-skip"),
            Message::Skip(id),
        );

        widget::container(
            widget::row::with_capacity(3)
                .spacing(spacing.space_s)
                .align_y(Alignment::Center)
                .push(left_slot)
                .push(primary_btn)
                .push(skip_slot),
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

        let mut col = widget::column::with_capacity(self.timers.len() + 2).spacing(space_xxs);

        col = col.push(self.header_row());

        if self.timers.is_empty() {
            col = col.push(self.empty_state());
            return col.into();
        }

        let dragging = self.dragging_index;

        let card_rows: Vec<Element<'_, Message>> = self
            .timers
            .iter()
            .enumerate()
            .map(|(i, timer)| {
                if dragging == Some(i) {
                    return widget::container(widget::Space::new().width(Length::Fill))
                        .height(Length::Fixed(4.0))
                        .width(Length::Fill)
                        .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                            let accent = Color::from(theme.cosmic().accent_color());
                            cosmic::iced::widget::container::Style {
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
                let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(3);

                items.push(
                    widget::icon::from_name("grip-lines-symbolic")
                        .size(16)
                        .icon()
                        .class(cosmic::theme::Svg::Custom(std::rc::Rc::new(
                            |theme: &cosmic::Theme| cosmic::iced::widget::svg::Style {
                                color: Some(theme.cosmic().palette.neutral_7.into()),
                            },
                        )))
                        .into(),
                );

                let info_col = widget::column::with_capacity(2)
                    .spacing(space_xxxs)
                    .push(widget::text::body(&timer.label))
                    .push(widget::text::title4(format_duration_hms(timer.remaining)));
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
                        let mut style = cosmic::iced::widget::container::Catalog::style(
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

        let timers_snapshot: Vec<(String, String)> = self
            .timers
            .iter()
            .map(|t| (t.label.clone(), format_duration_hms(t.remaining)))
            .collect();

        let reorder_list = ReorderList::new(cards, item_count, self.dragging_index)
            .on_start_drag(Message::StartDrag)
            .on_reorder(Message::Reorder)
            .on_finish(Message::FinishDrag)
            .on_cancel(Message::CancelDrag)
            .drag_icon(move |index, offset| {
                let (label, time_str) = timers_snapshot
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| ("Pomodoro".to_string(), String::new()));

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

                let card: Element<'static, ()> = widget::container(content)
                    .padding(8)
                    .width(Length::Fill)
                    .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                        let accent = Color::from(theme.cosmic().accent_color());
                        let mut style = cosmic::iced::widget::container::Catalog::style(
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

                (card, cosmic::iced::core::widget::tree::State::None, offset)
            });

        col = col.push(reorder_list);
        col.into()
    }

    /// Shared header: title + edit toggle (when timers exist) + add button.
    fn header_row(&self) -> Element<'_, Message> {
        let mut header = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("pomodoro-title")).width(Length::Fill));

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
                .on_press(Message::OpenSettings),
        );

        header.into()
    }

    /// Centered empty state: large icon + create button.
    fn empty_state(&self) -> Element<'_, Message> {
        let icon = widget::icon::icon(crate::app::bundled_icon(crate::app::POMODORO_ICON))
            .size(128)
            .class(cosmic::theme::Svg::Custom(std::rc::Rc::new(
                |theme: &cosmic::Theme| cosmic::iced::widget::svg::Style {
                    color: Some(theme.cosmic().palette.neutral_5.into()),
                },
            )));

        let empty_state = widget::column::with_capacity(2)
            .spacing(16)
            .align_x(Alignment::Center)
            .push(icon)
            .push(widget::button::suggested(fl!("create-pomodoro")).on_press(Message::OpenSettings));

        widget::container(empty_state)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Settings sidebar view
    pub fn settings_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(14).spacing(spacing);

        if let Some(_edit_id) = self.editing_id {
            // Editing existing pomodoro timer
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
            let mut actions = widget::row::with_capacity(3)
                .spacing(8)
                .push(widget::button::standard(fl!("cancel")).on_press(Message::CancelEditPomodoro))
                .push(widget::button::suggested(fl!("save")).on_press(Message::SaveEditPomodoro));
            if let Some(edit_id) = self.editing_id {
                actions = actions
                    .push(widget::button::destructive(fl!("delete")).on_press(Message::Delete(edit_id)));
            }
            col = col.push(actions);
        } else {
            // Add new timer
            col = col.push(
                widget::text_input(fl!("label-placeholder-pomodoro"), &self.edit_label)
                    .id(widget::Id::new("pomodoro-label-input"))
                    .on_input(Message::EditNewLabel),
            );
            // Sound for the timer about to be created. Without this the chosen
            // sound can't be set at creation time and every new pomodoro is "Bell".
            col = col.push(sound_selector_view(
                fl!("sound"),
                &self.edit_sound,
                Message::EditSound,
                Message::BrowseCustomSound,
            ));
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

/// Wrap content in a themed primary card container with the given padding and height.
///
/// Height is explicit rather than `Fill`: `flex_row` maps a `Fill` child to a
/// taffy `auto` size and then computes layout against the *available* height,
/// not the content height. Since `card_grid_view`'s column is itself `Fill`,
/// that available height is the whole page, so `align-items: stretch` inflates
/// the cards and pushes the timer grid off-screen.
/// Height of the bar area in the weekly focus chart.
const BAR_MAX_HEIGHT: f32 = 40.0;

fn themed_card(content: Element<'_, Message>, padding: u16, height: Length) -> Element<'_, Message> {
    widget::container(content)
        .padding(padding)
        .width(Length::Fill)
        .height(height)
        .class(cosmic::theme::Container::Custom(Box::new(|theme| {
            let mut style = cosmic::iced::widget::container::Catalog::style(
                theme,
                &cosmic::theme::Container::Primary,
            );
            style.border.radius = theme.cosmic().radius_s().into();
            style.background = Some(Color::from(theme.cosmic().bg_component_color()).into());
            style
        })))
        .into()
}
