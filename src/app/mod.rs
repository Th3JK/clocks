// SPDX-License-Identifier: MIT

mod dialogs;
mod helpers;
mod lifecycle;
mod persistence;
mod subscriptions;

use crate::config::Config;
use crate::pages::ContextPage;
use crate::pages::{alarm, pomodoro, stopwatch, timer, world_clocks};
use cosmic::cosmic_config;
use cosmic::widget::{about::About, menu, nav_bar};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../../resources/icons/hicolor/scalable/apps/icon.svg");

// --- Destructive action confirmation ---

#[derive(Debug, Clone)]
pub enum DestructiveAction {
    DeleteAlarm(u32),
    DeleteTimer(u32),
    DeleteWorldClock(u32),
    DeletePomodoro(u32),
    ClearStopwatchHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationCategory {
    DeleteAlarm,
    DeleteTimer,
    DeleteWorldClock,
    DeletePomodoro,
    ClearStopwatch,
}

// --- Model ---

pub struct AppModel {
    core: cosmic::Core,
    context_page: ContextPage,
    about: About,
    nav: nav_bar::Model,
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    config: Config,
    config_context: Option<cosmic_config::Config>,
    use_12h: bool,
    show_shortcuts_dialog: bool,

    // Confirmation dialog state
    pending_destructive_action: Option<DestructiveAction>,
    confirm_dialog_dont_show_again: bool,

    // Confirmation settings (mirrored from config)
    confirm_delete_alarm: bool,
    confirm_delete_timer: bool,
    confirm_delete_world_clock: bool,
    confirm_delete_pomodoro: bool,
    confirm_clear_stopwatch: bool,

    // Page states (each page owns its own MVU model)
    world_clocks: world_clocks::WorldClocksState,
    stopwatch: stopwatch::StopwatchState,
    alarm: alarm::AlarmState,
    timer: timer::TimerState,
    pomodoro: pomodoro::PomodoroState,

    // Last-active item IDs for keyboard shortcut targeting (session-only, not persisted)
    active_timer_id: Option<u32>,
    active_pomodoro_id: Option<u32>,

    // Audio stop handles for ringing alarms
    alarm_audio_stops: HashMap<u32, Arc<AtomicBool>>,
}

// --- Messages ---

#[derive(Debug, Clone)]
pub enum Message {
    LaunchUrl(String),
    ToggleContextPage(ContextPage),
    UpdateConfig(Config),
    Tick,
    WorldClocks(world_clocks::Message),
    Stopwatch(stopwatch::Message),
    Alarm(alarm::Message),
    Timer(timer::Message),
    Pomodoro(pomodoro::Message),
    CustomSoundSelected(CustomSoundTarget, String),
    SetTimeFormat(bool),
    // Keyboard shortcuts
    Quit,
    NavigateNext,
    NavigatePrev,
    NavigateTo(u16),
    PageShortcutSpace,
    PageShortcutEnter,
    PageShortcutDelete,
    PageShortcutCtrlN,
    PageShortcutSkip,
    ShowShortcutsDialog,
    CloseShortcutsDialog,
    // Confirmation dialogs
    ConfirmDestructiveAction,
    CancelDestructiveAction,
    ToggleConfirmDontShowAgain(bool),
    ToggleConfirmationSetting(ConfirmationCategory, bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomSoundTarget {
    Alarm,
    Timer,
    Pomodoro,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
    Settings,
    Shortcuts,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::Settings => Message::ToggleContextPage(ContextPage::Settings),
            MenuAction::Shortcuts => Message::ShowShortcutsDialog,
        }
    }
}
