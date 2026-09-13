// SPDX-License-Identifier: MIT
//
// Chess clock view: two tappable player panels, controls, and settings sidebar.

use super::Message;
use super::model::*;
use crate::components::format_duration_clock;
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

        // Title plus the active time control, e.g. "Blitz 3+2" or a bare "5+3".
        let time_control = match self.preset_name() {
            Some(name) => name,
            None => fl!(
                "chess-time-control",
                base = self.base_minutes.to_string(),
                increment = self.increment_secs.to_string()
            ),
        };
        let title_col = widget::column::with_capacity(2)
            .push(widget::text::title3(fl!("chess-title")))
            .push(widget::text::caption(time_control))
            .width(Length::Fill);

        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(title_col)
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

        // Controls: three fixed slots so the 48px primary sits dead centre.
        // Centring just [primary, reset] would push the primary left by half the
        // reset button, which is what the other pages use a spacer to avoid.
        let (pause_icon, pause_tip) = if self.running {
            ("media-playback-pause-symbolic", fl!("tooltip-pause"))
        } else {
            ("media-playback-start-symbolic", fl!("tooltip-resume"))
        };

        // An invisible button rather than a Space, so the slot matches the real
        // button's metrics exactly.
        let spacer = |size: u16| -> Element<'_, Message> {
            widget::button::custom(
                widget::container(widget::Space::new())
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(size)
                    .height(size),
            )
            .class(cosmic::theme::Button::Custom {
                active: Box::new(|_, _| widget::button::Style::default()),
                disabled: Box::new(|_| widget::button::Style::default()),
                hovered: Box::new(|_, _| widget::button::Style::default()),
                pressed: Box::new(|_, _| widget::button::Style::default()),
            })
            .into()
        };

        // Once a player has flagged the game is over, so the primary is replaced
        // by a spacer of the same size — reset keeps its position, no jump.
        let primary_slot: Element<'_, Message> = if self.flagged.is_none() {
            widget::tooltip(
                widget::button::custom(
                    widget::container(widget::icon::from_name(pause_icon).size(24).icon())
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .width(48)
                        .height(48),
                )
                .class(if self.running {
                    cosmic::theme::Button::Standard
                } else {
                    cosmic::theme::Button::Suggested
                })
                .on_press(Message::PauseToggle),
                widget::text::body(pause_tip),
                widget::tooltip::Position::Top,
            )
            .into()
        } else {
            spacer(48)
        };

        let reset_slot: Element<'_, Message> = widget::tooltip(
            widget::button::custom(
                widget::container(
                    widget::icon::from_name("edit-undo-symbolic").size(16).icon(),
                )
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .width(32)
                .height(32),
            )
            .class(cosmic::theme::Button::Standard)
            .on_press(Message::Reset),
            widget::text::body(fl!("tooltip-reset")),
            widget::tooltip::Position::Top,
        )
        .into();

        let controls = widget::container(
            widget::row::with_capacity(3)
                .spacing(spacing.space_s)
                .align_y(Alignment::Center)
                .push(spacer(32))
                .push(primary_slot)
                .push(reset_slot),
        )
        .align_x(Alignment::Center)
        .width(Length::Fill);

        // Status line. Before the first move this says how to start rather than
        // the misleading "Paused".
        let status = if let Some(flagged) = self.flagged {
            widget::text::body(fl!(
                "chess-winner",
                player = flagged.opponent().display_name()
            ))
        } else if self.running {
            widget::text::caption(fl!("chess-turn", player = self.current_turn.display_name()))
        } else if self.is_fresh() {
            widget::text::caption(fl!(
                "chess-press-to-start",
                player = self.current_turn.display_name()
            ))
        } else {
            widget::text::caption(fl!("chess-paused"))
        };
        let status = widget::container(status)
            .align_x(Alignment::Center)
            .width(Length::Fill);

        let mut col = widget::column::with_capacity(5)
            .spacing(spacing.space_s)
            .width(Length::Fill)
            .height(Length::Fill)
            .push(header)
            .push(panels)
            .push(status)
            .push(controls);

        // Hidden until the first move — an empty box would just re-add the
        // blankness this page already suffered from.
        if !self.moves.is_empty() {
            col = col.push(self.move_list());
        }

        col.into()
    }

    /// Scoresheet of completed moves: one row per move number, White and Black
    /// side by side.
    fn move_list(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let cell = |s: String| {
            widget::container(widget::text::caption(s))
                .width(Length::Fill)
                .align_x(Alignment::Center)
        };

        let header = widget::row::with_capacity(3)
            .push(cell(fl!("chess-move-number")))
            .push(cell(fl!("chess-white")))
            .push(cell(fl!("chess-black")))
            .width(Length::Fill);

        // White always moves first, so consecutive pairs are (White, Black) and
        // a trailing odd move is White's.
        let rows: Vec<Element<'_, Message>> = self
            .moves
            .chunks(2)
            .enumerate()
            .map(|(i, pair)| {
                let secs = |d: std::time::Duration| format!("{:.1}s", d.as_secs_f32());
                widget::row::with_capacity(3)
                    .push(cell((i + 1).to_string()))
                    .push(cell(pair.first().map(|m| secs(m.elapsed)).unwrap_or_default()))
                    .push(cell(pair.get(1).map(|m| secs(m.elapsed)).unwrap_or_default()))
                    .width(Length::Fill)
                    .into()
            })
            .collect();

        let body = widget::column::with_children(rows)
            .spacing(spacing.space_xxxs)
            .width(Length::Fill);

        let list = widget::column::with_capacity(3)
            .spacing(spacing.space_xxs)
            .push(header)
            .push(widget::divider::horizontal::default())
            // The scroll area takes whatever vertical space is left after the
            // clocks and controls, so the sheet runs to the bottom of the page
            // and only scrolls once it actually overflows.
            .push(widget::scrollable(body).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill);

        widget::container(list)
            .padding(spacing.space_s)
            .width(Length::Fill)
            .height(Length::Fill)
            .class(cosmic::theme::Container::Custom(Box::new(|theme| {
                let cosmic = theme.cosmic();
                let mut style = cosmic::iced::widget::container::Catalog::style(
                    theme,
                    &cosmic::theme::Container::Primary,
                );
                style.border.radius = cosmic.radius_s().into();
                style.background = Some(Color::from(cosmic.bg_component_color()).into());
                style
            })))
            .into()
    }

    /// A large tappable panel showing one player's remaining time.
    fn player_panel(&self, player: Player) -> Element<'_, Message> {
        let is_turn = self.running && self.current_turn == player;
        let is_flagged = self.flagged == Some(player);

        let remaining = self.remaining_of(player);
        let cosmic_theme = cosmic::theme::active();

        // Low time gets emphasis: destructive under 10s, warning-ish under 30s.
        let time_color: Option<Color> = if is_flagged || remaining.as_secs() < 10 {
            Some(cosmic_theme.cosmic().destructive_color().into())
        } else if remaining.as_secs() < 30 {
            Some(cosmic_theme.cosmic().warning_color().into())
        } else {
            None
        };

        // Sized to fit the card at its narrowest: five characters at 44px of a
        // monospace face, or seven once a game passes an hour.
        let mut time_text = widget::text(format_duration_clock(remaining))
            .size(44.0)
            .font(light_font())
            .width(Length::Fill)
            .align_x(Alignment::Center);
        if let Some(c) = time_color {
            time_text = time_text.class(cosmic::theme::Text::Color(c));
        }

        // Remaining-time bar. Fills the panel and reads at a glance from across
        // a table, which a bare number does not.
        let frac = self.remaining_fraction(player);
        // Portions are integers and must never both be zero, or the split is a
        // division by zero.
        let filled_portion = ((frac * 1000.0) as u16).max(1);
        let empty_portion = (1000u16.saturating_sub(filled_portion)).max(1);
        let bar_fill: Color = match time_color {
            Some(c) => c,
            None if is_turn => cosmic_theme.cosmic().accent_color().into(),
            None => cosmic_theme.cosmic().palette.neutral_6.into(),
        };
        let bar = widget::container(widget::Space::new())
            .width(Length::FillPortion(filled_portion))
            .height(Length::Fixed(6.0))
            .class(cosmic::theme::Container::Custom(Box::new(move |theme| {
                cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Background::Color(bar_fill)),
                    border: cosmic::iced::Border {
                        radius: theme.cosmic().radius_xs().into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })));
        // The unfilled remainder, so the bar shrinks rather than the track moving.
        let bar_row = widget::row::with_capacity(2)
            .push(bar)
            .push(widget::Space::new().width(Length::FillPortion(empty_portion)))
            .width(Length::Fill);

        // Move count and how long the last move took.
        let moves = self.moves_of(player);
        let move_line = match self.last_move_of(player) {
            Some(d) => fl!(
                "chess-moves-last",
                moves = moves.to_string(),
                seconds = format!("{:.1}", d.as_secs_f32())
            ),
            None => fl!("chess-moves", moves = moves.to_string()),
        };

        let mut content = widget::column::with_capacity(5)
            .spacing(8)
            .align_x(Alignment::Center)
            .push(widget::text::title4(player.display_name()))
            .push(time_text)
            .push(bar_row)
            .push(widget::text::caption(move_line));

        // Outcome badge once someone has flagged.
        if let Some(flagged) = self.flagged {
            let badge = if flagged == player {
                widget::text::caption(fl!("chess-lost-on-time")).class(
                    cosmic::theme::Text::Color(cosmic_theme.cosmic().destructive_color().into()),
                )
            } else {
                widget::text::caption(fl!("chess-won")).class(cosmic::theme::Text::Color(
                    cosmic_theme.cosmic().accent_color().into(),
                ))
            };
            content = content.push(badge);
        }

        let panel = widget::container(content)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding(cosmic::theme::spacing().space_l)
            .width(Length::Fill)
            .height(Length::Fixed(220.0))
            .class(cosmic::theme::Container::Custom(Box::new(move |theme| {
                let cosmic = theme.cosmic();
                let mut style = cosmic::iced::widget::container::Catalog::style(
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
