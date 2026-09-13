// SPDX-License-Identifier: MIT

//! Shared library behind the `clocks` binaries.
//!
//! The GUI is one consumer of this crate; the background daemon that owns
//! alarm scheduling is the other. Anything both need -- the config schema,
//! the page models, audio playback, notifications, i18n -- lives here rather
//! than inside the GUI binary.

/// D-Bus name, config id and state id. Shared so the GUI and the daemon cannot
/// drift apart on which config they are talking about.
pub const APP_ID: &str = "dev.th3jk.clocks";

pub mod app;
pub mod audio;
pub mod components;
pub mod config;
pub mod flags;
pub mod i18n;
pub mod ipc;
pub mod pages;
pub mod quick_action;
pub mod runtime;
pub mod scheduler;
pub mod sounds;
pub mod time_format;
