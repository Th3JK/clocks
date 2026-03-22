// SPDX-License-Identifier: MIT

use crate::components::{format_duration, format_duration_parts};
use crate::fl;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct LapEntry {
    pub id: u32,
    pub lap_time: Duration,
    pub delta: i64,
    pub is_fastest: bool,
    pub is_slowest: bool,
}

#[derive(Debug, Clone)]
pub struct StopwatchRecord {
    pub id: u32,
    pub label: String,
    pub total_elapsed: Duration,
    pub laps: Vec<LapEntry>,
}

pub struct StopwatchState {
    pub elapsed: Duration,
    pub is_running: bool,
    pub start_instant: Option<Instant>,
    pub accumulated: Duration,
    pub laps: Vec<LapEntry>,
    pub lap_start: Duration,
    pub next_lap_id: u32,
    // History
    pub history: Vec<StopwatchRecord>,
    pub next_history_id: u32,
    pub current_label: String,
    pub current_session_id: Option<u32>,
}

impl Default for StopwatchState {
    fn default() -> Self {
        Self {
            elapsed: Duration::ZERO,
            is_running: false,
            start_instant: None,
            accumulated: Duration::ZERO,
            laps: Vec::new(),
            lap_start: Duration::ZERO,
            next_lap_id: 1,
            history: Vec::new(),
            next_history_id: 1,
            current_label: String::new(),
            current_session_id: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Start,
    Stop,
    Reset,
    Lap,
    Tick,
    // History
    EditHistoryLabel(u32, String),
    DeleteHistory(u32),
    ResumeFromHistory(u32),
    ClearHistory,
    OpenHistory,
}

impl StopwatchState {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Start => {
                self.is_running = true;
                self.start_instant = Some(Instant::now());
                // Create history entry for new session if none exists
                if self.current_session_id.is_none() {
                    let id = self.next_history_id;
                    self.next_history_id += 1;
                    self.current_session_id = Some(id);
                    self.history.push(StopwatchRecord {
                        id,
                        label: if self.current_label.is_empty() {
                            fl!("session-default", id = id.to_string())
                        } else {
                            self.current_label.clone()
                        },
                        total_elapsed: Duration::ZERO,
                        laps: Vec::new(),
                    });
                }
            }
            Message::Stop => {
                if let Some(start) = self.start_instant.take() {
                    self.accumulated += start.elapsed();
                }
                self.is_running = false;
                self.elapsed = self.accumulated;
                // Update current session
                if let Some(session_id) = self.current_session_id {
                    if let Some(record) = self.history.iter_mut().find(|r| r.id == session_id) {
                        record.total_elapsed = self.elapsed;
                        record.laps = self.laps.clone();
                    }
                }
            }
            Message::Reset => {
                // Finalize current session in history
                if let Some(session_id) = self.current_session_id {
                    let total = self.current_elapsed();
                    if let Some(record) = self.history.iter_mut().find(|r| r.id == session_id) {
                        record.total_elapsed = total;
                        record.laps = self.laps.clone();
                    }
                    // Remove the record if nothing was recorded
                    if total == Duration::ZERO {
                        self.history.retain(|r| r.id != session_id);
                    }
                }
                // Reset stopwatch state but keep history
                let history = std::mem::take(&mut self.history);
                let next_history_id = self.next_history_id;
                *self = Self::default();
                self.history = history;
                self.next_history_id = next_history_id;
            }
            Message::Lap => {
                let current_elapsed = self.current_elapsed();
                let lap_time = current_elapsed.saturating_sub(self.lap_start);

                let delta = if let Some(prev) = self.laps.last() {
                    lap_time.as_millis() as i64 - prev.lap_time.as_millis() as i64
                } else {
                    0
                };

                self.laps.push(LapEntry {
                    id: self.next_lap_id,
                    lap_time,
                    delta,
                    is_fastest: false,
                    is_slowest: false,
                });
                self.next_lap_id += 1;
                self.lap_start = current_elapsed;

                if self.laps.len() >= 2 {
                    let min = self.laps.iter().map(|l| l.lap_time).min().unwrap();
                    let max = self.laps.iter().map(|l| l.lap_time).max().unwrap();
                    for lap in &mut self.laps {
                        lap.is_fastest = lap.lap_time == min;
                        lap.is_slowest = lap.lap_time == max;
                    }
                }
                // Update current session history entry
                if let Some(session_id) = self.current_session_id {
                    if let Some(record) = self.history.iter_mut().find(|r| r.id == session_id) {
                        record.total_elapsed = current_elapsed;
                        record.laps = self.laps.clone();
                    }
                }
            }
            Message::Tick => {
                self.elapsed = self.current_elapsed();
            }
            Message::EditHistoryLabel(id, label) => {
                if let Some(record) = self.history.iter_mut().find(|r| r.id == id) {
                    record.label = label;
                }
            }
            Message::DeleteHistory(id) => {
                self.history.retain(|r| r.id != id);
            }
            Message::ResumeFromHistory(id) => {
                if let Some(record) = self.history.iter().find(|r| r.id == id) {
                    self.accumulated = record.total_elapsed;
                    self.elapsed = record.total_elapsed;
                    self.laps = record.laps.clone();
                    self.next_lap_id = self.laps.len() as u32 + 1;
                    self.lap_start = record.total_elapsed;
                    self.is_running = false;
                    self.start_instant = None;
                    self.current_label = record.label.clone();
                    self.current_session_id = Some(id);
                }
            }
            Message::ClearHistory => {
                self.history.clear();
            }
            Message::OpenHistory => {
                // Handled in app.rs
            }
        }
    }

    fn current_elapsed(&self) -> Duration {
        if let Some(start) = self.start_instant {
            self.accumulated + start.elapsed()
        } else {
            self.accumulated
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let spacing = 12;

        let mut col = widget::column::with_capacity(5)
            .spacing(spacing)
            .align_x(Alignment::Center)
            .width(Length::Fill);

        // Page header
        let header = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::text::title3(fl!("stopwatch-title")).width(Length::Fill))
            .push(
                widget::button::icon(widget::icon::from_name("addressbook-symbolic"))
                    .on_press(Message::OpenHistory),
            );
        col = col.push(header);

        // Main time display — seconds bold, rest lighter
        let (prefix, seconds, suffix) = format_duration_parts(self.elapsed);
        let time_row = widget::row::with_capacity(3)
            .align_y(Alignment::Center)
            .push(widget::text::title1(prefix).font(cosmic::font::default()))
            .push(widget::text::title1(seconds).font(cosmic::font::bold()))
            .push(widget::text::title1(suffix).font(cosmic::font::default()));
        col = col.push(
            widget::container(time_row)
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .padding(24),
        );

        // Control buttons
        let mut controls = widget::row::with_capacity(3)
            .spacing(spacing)
            .align_y(Alignment::Center);

        if self.is_running {
            controls = controls.push(widget::button::destructive(fl!("stop")).on_press(Message::Stop));
            controls = controls.push(widget::button::standard(fl!("lap")).on_press(Message::Lap));
        } else if self.elapsed > Duration::ZERO {
            controls = controls.push(widget::button::suggested(fl!("resume")).on_press(Message::Start));
            controls = controls.push(widget::button::standard(fl!("reset")).on_press(Message::Reset));
        } else {
            controls = controls.push(widget::button::suggested(fl!("start")).on_press(Message::Start));
        }

        col = col.push(
            widget::container(controls)
                .align_x(Alignment::Center)
                .width(Length::Fill),
        );

        // Laps display
        if !self.laps.is_empty() {
            col = col.push(widget::divider::horizontal::default());

            let mut laps_col = widget::column::with_capacity(self.laps.len()).spacing(4);
            let green = cosmic::iced::Color::from_rgb(0.2, 0.8, 0.2);
            let red = cosmic::iced::Color::from_rgb(0.9, 0.3, 0.3);

            for (i, lap) in self.laps.iter().enumerate().rev() {
                let lap_label = format!("{}: {}", fl!("lap-entry", id = lap.id.to_string()), format_duration(lap.lap_time));
                let mut row = widget::row::with_capacity(3)
                    .spacing(6)
                    .align_y(Alignment::Center);
                row = row.push(widget::text::body(lap_label));

                if i > 0 {
                    let (delta_str, delta_color) = if lap.delta < 0 {
                        (
                            format!(
                                "-{}",
                                format_duration(Duration::from_millis((-lap.delta) as u64))
                            ),
                            green,
                        )
                    } else {
                        (
                            format!(
                                "+{}",
                                format_duration(Duration::from_millis(lap.delta as u64))
                            ),
                            red,
                        )
                    };
                    row = row.push(
                        widget::text::body(delta_str)
                            .class(cosmic::theme::Text::Color(delta_color)),
                    );
                }

                if lap.is_fastest {
                    row = row.push(
                        widget::text::caption(fl!("fastest"))
                            .class(cosmic::theme::Text::Color(green)),
                    );
                } else if lap.is_slowest {
                    row = row.push(
                        widget::text::caption(fl!("slowest"))
                            .class(cosmic::theme::Text::Color(red)),
                    );
                }

                laps_col = laps_col.push(row);
            }

            col = col.push(laps_col);
        }

        col.into()
    }

    /// History sidebar view
    pub fn history_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(self.history.len() + 2).spacing(spacing);

        if self.history.is_empty() {
            col = col.push(widget::text::body(fl!("no-history")));
            col = col.push(widget::text::caption(fl!("history-hint")));
        } else {
            col = col.push(
                widget::button::destructive(fl!("clear-all-history")).on_press(Message::ClearHistory),
            );

            for record in self.history.iter().rev() {
                let id = record.id;
                col = col.push(widget::divider::horizontal::default());

                // Editable label
                col = col.push(
                    widget::text_input(fl!("session-label"), &record.label)
                        .on_input(move |l| Message::EditHistoryLabel(id, l)),
                );

                col = col.push(widget::text::body(
                    fl!("total-time", time = format_duration(record.total_elapsed))
                ));

                if !record.laps.is_empty() {
                    col = col.push(widget::text::caption(
                        fl!("laps-recorded", count = record.laps.len().to_string())
                    ));
                }

                let actions = widget::row::with_capacity(2)
                    .spacing(8)
                    .push(
                        widget::button::suggested(fl!("resume"))
                            .on_press(Message::ResumeFromHistory(id)),
                    )
                    .push(
                        widget::button::destructive(fl!("delete")).on_press(Message::DeleteHistory(id)),
                    );
                col = col.push(actions);
            }
        }

        col.into()
    }
}
