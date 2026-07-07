// SPDX-License-Identifier: MIT
//
// Chess clock view: two tappable player panels, controls, and settings sidebar.

use super::Message;
use super::model::*;
use crate::components::format_duration_hms;
use crate::fl;
use cosmic::iced::font::Weight;
use cosmic::iced::{Alignment, Border, Color, Length};
use cosmic::prelude::*;
use cosmic::widget;

fn light_font() -> cosmic::iced::Font {
    cosmic::iced::Font {
        weight: Weight::Light,
        ..cosmic::font::default()
    }
}

impl ChessState {
    pub fn view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("chess-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("emblem-system-symbolic"))
                    .tooltip(fl!("chess-settings"))
                    .on_press(Message::OpenSettings),
            );

        let panels = widget::flex_row(vec![
            self.player_panel(Player::White),
            self.player_panel(Player::Black),
        ])
        .spacing(spacing.space_s as u16)
        .min_item_width(220.0)
        .width(Length::Fill);

        // Center controls: pause/resume + reset.
        let (pause_icon, pause_tip) = if self.running {
            ("media-playback-pause-symbolic", fl!("tooltip-pause"))
        } else {
            ("media-playback-start-symbolic", fl!("tooltip-resume"))
        };
        let mut controls = widget::row::with_capacity(2).spacing(spacing.space_s);
        if self.flagged.is_none() {
            controls = controls.push(
                widget::button::icon(widget::icon::from_name(pause_icon).size(24))
                    .tooltip(pause_tip)
                    .on_press(Message::PauseToggle),
            );
        }
        controls = controls.push(
            widget::button::icon(widget::icon::from_name("edit-undo-symbolic").size(24))
                .tooltip(fl!("tooltip-reset"))
                .on_press(Message::Reset),
        );
        let controls = widget::container(controls)
            .align_x(Alignment::Center)
            .width(Length::Fill);

        // Status line.
        let status = if let Some(flagged) = self.flagged {
            widget::text::body(fl!(
                "chess-winner",
                player = flagged.opponent().display_name()
            ))
        } else if self.running {
            widget::text::caption(fl!("chess-turn", player = self.current_turn.display_name()))
        } else {
            widget::text::caption(fl!("chess-paused"))
        };
        let status = widget::container(status)
            .align_x(Alignment::Center)
            .width(Length::Fill);

        widget::column::with_capacity(4)
            .spacing(spacing.space_s)
            .width(Length::Fill)
            .height(Length::Fill)
            .push(header)
            .push(panels)
            .push(status)
            .push(controls)
            .into()
    }

    /// A large tappable panel showing one player's remaining time.
    fn player_panel(&self, player: Player) -> Element<'_, Message> {
        let is_turn = self.running && self.current_turn == player;
        let is_flagged = self.flagged == Some(player);

        let time_text = widget::text(format_duration_hms(self.remaining_of(player)))
            .size(48.0)
            .font(light_font());

        let content = widget::column::with_capacity(2)
            .spacing(8)
            .align_x(Alignment::Center)
            .push(widget::text::title4(player.display_name()))
            .push(time_text);

        let panel = widget::container(content)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding(cosmic::theme::spacing().space_l)
            .width(Length::Fill)
            .height(Length::Fixed(220.0))
            .class(cosmic::theme::Container::Custom(Box::new(move |theme| {
                let cosmic = theme.cosmic();
                let mut style = cosmic::iced_widget::container::Catalog::style(
                    theme,
                    &cosmic::theme::Container::Primary,
                );
                style.border.radius = cosmic.radius_s().into();
                style.background = Some(Color::from(cosmic.bg_component_color()).into());
                if is_flagged {
                    style.border = Border {
                        color: Color::from(cosmic.destructive_color()),
                        width: 2.0,
                        radius: cosmic.radius_s().into(),
                    };
                } else if is_turn {
                    style.border = Border {
                        color: Color::from(cosmic.accent_color()),
                        width: 2.0,
                        radius: cosmic.radius_s().into(),
                    };
                }
                style
            })));

        widget::mouse_area(panel)
            .on_press(Message::TapPlayer(player))
            .into()
    }

    /// Settings sidebar: presets + base time / increment steppers.
    pub fn settings_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(10).spacing(spacing);

        col = col.push(widget::text::title4(fl!("chess-settings")));

        // Presets
        col = col.push(widget::text::body(fl!("chess-presets")));
        let presets = widget::flex_row(vec![
            widget::button::standard(fl!("chess-preset-bullet"))
                .on_press(Message::ApplyPreset(1, 0))
                .into(),
            widget::button::standard(fl!("chess-preset-blitz"))
                .on_press(Message::ApplyPreset(3, 2))
                .into(),
            widget::button::standard(fl!("chess-preset-rapid"))
                .on_press(Message::ApplyPreset(10, 0))
                .into(),
            widget::button::standard(fl!("chess-preset-classical"))
                .on_press(Message::ApplyPreset(30, 0))
                .into(),
        ])
        .spacing(4)
        .min_item_width(80.0);
        col = col.push(presets);

        col = col.push(widget::divider::horizontal::default());

        // Base minutes
        let b = self.edit_base_minutes;
        let base_row = widget::row::with_capacity(4)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(widget::text::body(fl!("chess-base-time")).width(Length::Fixed(120.0)))
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                    .on_press(Message::EditBaseMinutes(b.saturating_sub(1))),
            )
            .push(widget::text::body(fl!("minutes-value", value = b.to_string())))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .on_press(Message::EditBaseMinutes(b + 1)),
            );
        col = col.push(base_row);

        // Increment seconds
        let inc = self.edit_increment_secs;
        let inc_row = widget::row::with_capacity(4)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(widget::text::body(fl!("chess-increment")).width(Length::Fixed(120.0)))
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                    .on_press(Message::EditIncrementSecs(inc.saturating_sub(1))),
            )
            .push(widget::text::body(fl!("seconds-value", value = inc.to_string())))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .on_press(Message::EditIncrementSecs(inc + 1)),
            );
        col = col.push(inc_row);

        col = col.push(widget::divider::horizontal::default());
        col = col.push(widget::text::caption(fl!("chess-apply-hint")));
        col = col.push(
            widget::button::suggested(fl!("chess-apply")).on_press(Message::ApplySettings),
        );

        col.into()
    }
}
