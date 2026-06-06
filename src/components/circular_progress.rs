// SPDX-License-Identifier: MIT
//
// Circular progress indicator using iced Canvas.

use cosmic::iced::{self, mouse, Color, Length, Rectangle};
use cosmic::iced_widget::canvas::{self, Frame, Path, Stroke};
use cosmic::prelude::*;
use std::f32::consts::PI;

/// A circular progress arc indicator.
///
/// `progress` is 0.0 (empty) to 1.0 (full).
pub struct CircularProgress {
    progress: f32,
    track_color: Color,
    fill_color: Color,
    size: f32,
    stroke_width: f32,
}

impl CircularProgress {
    pub fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            track_color: Color::from_rgba(0.5, 0.5, 0.5, 0.2),
            fill_color: Color::from_rgb(0.3, 0.6, 1.0),
            size: 120.0,
            stroke_width: 6.0,
        }
    }

    pub fn track_color(mut self, color: Color) -> Self {
        self.track_color = color;
        self
    }

    pub fn fill_color(mut self, color: Color) -> Self {
        self.fill_color = color;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    pub fn view<M: 'static>(self) -> Element<'static, M> {
        let size = self.size;
        cosmic::iced_widget::Canvas::<Self, M, cosmic::Theme, cosmic::Renderer>::new(self)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .into()
    }
}

impl<M> canvas::Program<M, cosmic::Theme, cosmic::Renderer> for CircularProgress {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &cosmic::Renderer,
        _theme: &cosmic::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let center = iced::Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = (bounds.width.min(bounds.height) / 2.0) - self.stroke_width;

        // Track (full circle, neutral background)
        let track = Path::circle(center, radius);
        frame.stroke(
            &track,
            Stroke::default()
                .with_width(self.stroke_width)
                .with_color(self.track_color),
        );

        // Progress arc (from 12 o'clock, clockwise)
        if self.progress > 0.001 {
            let start_angle = -PI / 2.0;
            let sweep = self.progress * 2.0 * PI;
            let end_angle = start_angle + sweep;

            let arc_path = Path::new(|builder| {
                builder.arc(canvas::path::Arc {
                    center,
                    radius,
                    start_angle: iced::Radians(start_angle),
                    end_angle: iced::Radians(end_angle),
                });
            });

            frame.stroke(
                &arc_path,
                Stroke::default()
                    .with_width(self.stroke_width)
                    .with_color(self.fill_color)
                    .with_line_cap(canvas::LineCap::Round),
            );
        }

        vec![frame.into_geometry()]
    }
}
