// SPDX-License-Identifier: MIT

//! Shared library behind the `clocks` binaries.
//!
//! The GUI is one consumer of this crate; the background daemon that owns
//! alarm scheduling is the other. Anything both need -- the config schema,
//! the page models, audio playback, notifications, i18n -- lives here rather
//! than inside the GUI binary.

pub mod app;
pub mod audio;
pub mod components;
pub mod config;
pub mod i18n;
pub mod pages;
pub mod quick_action;
pub mod sounds;
pub mod time_format;
