// SPDX-License-Identifier: MIT

mod dialogs;
mod helpers;
mod lifecycle;
pub mod persistence;
mod subscriptions;

use crate::config::Config;
use crate::pages::ContextPage;
use crate::pages::{alarm, chess, countdown, pomodoro, stopwatch, timer, workout, world_clocks};
use cosmic::widget::{about::About, menu, nav_bar, toaster};
use std::collections::HashMap;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../../resources/icons/hicolor/scalable/apps/icon.svg");
/// Timer and Pomodoro have no reliable freedesktop symbolic icon (`timer-symbolic`
/// is absent from most themes), so both are bundled and loaded from bytes.
pub(crate) const TIMER_ICON: &[u8] =
    include_bytes!("../../resources/icons/hicolor/scalable/apps/timer-symbolic.svg");
pub(crate) const POMODORO_ICON: &[u8] =
    include_bytes!("../../resources/icons/hicolor/scalable/apps/pomodoro-symbolic.svg");
pub(crate) const COUNTDOWN_ICON: &[u8] =
    include_bytes!("../../resources/icons/hicolor/scalable/apps/countdown-symbolic.svg");

/// Build a themed icon handle from bundled SVG bytes.
///
/// `icon::from_svg_bytes` leaves `symbolic: false`, which stops libcosmic from
/// recolouring the glyph for the active theme — it would render black on dark
/// backgrounds. Setting the flag opts the icon into theme tinting.
pub(crate) fn bundled_icon(bytes: &'static [u8]) -> cosmic::widget::icon::Handle {
    let mut handle = cosmic::widget::icon::from_svg_bytes(bytes);
    handle.symbolic = true;
    handle
}

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
    /// The stored preference. Persisted as-is so choosing System survives.
    time_format: crate::time_format::TimeFormat,
    /// `time_format` resolved to a concrete flag, recomputed whenever the
    /// setting changes. Kept separate so the resolved value is never written
    /// back over the stored preference.
    use_12h: bool,
    show_shortcuts_dialog: bool,
    /// Sidebar order and hidden set. Persisted by page key, never by
    /// `nav_bar::Entity` - those go stale the moment the model is rebuilt.
    nav_order: Vec<crate::pages::Page>,
    nav_hidden: Vec<crate::pages::Page>,
    /// Quick-action palette state (session-only).
    show_palette: bool,
    palette_input: String,

    // Confirmation dialog state
    pending_destructive_action: Option<DestructiveAction>,
    confirm_dialog_dont_show_again: bool,

    // Confirmation settings (mirrored from config)
    confirm_delete_alarm: bool,
    confirm_delete_timer: bool,
    confirm_delete_world_clock: bool,
    confirm_delete_pomodoro: bool,
    confirm_clear_stopwatch: bool,
    auto_sort_alarms: bool,
    auto_sort_world_clocks: bool,
    auto_clear_stopwatch_history: bool,

    // Page states (each page owns its own MVU model)
    world_clocks: world_clocks::WorldClocksState,
    stopwatch: stopwatch::StopwatchState,
    alarm: alarm::AlarmState,
    timer: timer::TimerState,
    pomodoro: pomodoro::PomodoroState,
    chess: chess::ChessState,
    workout: workout::WorkoutState,
    countdown: countdown::CountdownState,

    // Last-active item IDs for keyboard shortcut targeting (session-only, not persisted)
    active_timer_id: Option<u32>,
    active_pomodoro_id: Option<u32>,

    // Toast notifications
    toasts: toaster::Toasts<Message>,
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
    Chess(chess::Message),
    Workout(workout::Message),
    Countdown(countdown::Message),
    CustomSoundSelected(CustomSoundTarget, String),
    SetTimeFormat(crate::time_format::TimeFormat),
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
    // Quick-action palette
    OpenPalette,
    ClosePalette,
    PaletteInput(String),
    PaletteSubmit,
    PaletteRun(crate::quick_action::QuickAction),
    // Sidebar customisation
    ToggleNavPage(crate::pages::Page, bool),
    /// Move the sidebar page at the first index to the second.
    MoveNavPage(usize, usize),
    /// The daemon's runtime state changed: something started or stopped ringing.
    UpdateRuntime(crate::runtime::RuntimeState),
    // Confirmation dialogs
    ConfirmDestructiveAction,
    CancelDestructiveAction,
    ToggleConfirmDontShowAgain(bool),
    ToggleConfirmationSetting(ConfirmationCategory, bool),
    // Toast notifications
    CloseToast(toaster::ToastId),
    // CSV export result (toast message text)
    ExportFinished(String),
    // Auto-sorting
    SetAutoSortAlarms(bool),
    SetAutoSortWorldClocks(bool),
    SetAutoClearStopwatchHistory(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomSoundTarget {
    Alarm,
    Timer,
    Pomodoro,
    Workout,
    Countdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
    Settings,
    Shortcuts,
    QuickAction,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::Settings => Message::ToggleContextPage(ContextPage::Settings),
            MenuAction::Shortcuts => Message::ShowShortcutsDialog,
            MenuAction::QuickAction => Message::OpenPalette,
        }
    }
}
