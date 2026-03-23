// SPDX-License-Identifier: MIT

use crate::audio;
use crate::config::{
    Config, PomodoroDefaults, SavedAlarm, SavedClock, SavedPomodoro, SavedRepeatMode, SavedTimer,
};
use crate::fl;
use crate::pages::{ContextPage, Page, alarm, pomodoro, stopwatch, timer, world_clocks};
use chrono::{Datelike, Local, Timelike};
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::keyboard::{self, key::Named, Key};
use cosmic::iced::Length;
use cosmic::iced::Subscription;
use cosmic::iced_futures::event::listen_raw;
use cosmic::widget::{self, about::About, icon, menu, nav_bar};
use cosmic::{iced_futures, prelude::*};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");

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

// --- Application trait (View + Update lifecycle) ---

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "dev.th3jk.clocks";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let mut nav = nav_bar::Model::default();

        nav.insert()
            .text(fl!("nav-world-clocks"))
            .data::<Page>(Page::WorldClocks)
            .icon(icon::from_name("preferences-system-time-symbolic"))
            .activate();

        nav.insert()
            .text(fl!("nav-stopwatch"))
            .data::<Page>(Page::Stopwatch)
            .icon(icon::from_name("media-playback-start-symbolic"));

        nav.insert()
            .text(fl!("nav-alarm"))
            .data::<Page>(Page::Alarm)
            .icon(icon::from_name("alarm-symbolic"));

        nav.insert()
            .text(fl!("nav-timer"))
            .data::<Page>(Page::Timer)
            .icon(icon::from_name("appointment-soon-symbolic"));

        nav.insert()
            .text(fl!("nav-pomodoro"))
            .data::<Page>(Page::Pomodoro)
            .icon(icon::from_name("appointment-soon-symbolic"));

        let about = About::default()
            .name(fl!("app-title"))
            .icon(widget::icon::from_svg_bytes(APP_ICON))
            .version(env!("CARGO_PKG_VERSION"))
            .links([(fl!("repository"), REPOSITORY)])
            .license(env!("CARGO_PKG_LICENSE"));

        let config_context = cosmic_config::Config::new(Self::APP_ID, Config::VERSION).ok();
        let config = config_context
            .as_ref()
            .map(|ctx| match Config::get_entry(ctx) {
                Ok(config) => config,
                Err((_errors, config)) => config,
            })
            .unwrap_or_default();

        // Restore state from config
        let world_clocks = restore_world_clocks(&config);
        let alarm = restore_alarms(&config);
        let timer = restore_timers(&config);
        let pomodoro = restore_pomodoros(&config);

        let use_12h = config.use_12h;
        let confirm_delete_alarm = config.confirm_delete_alarm;
        let confirm_delete_timer = config.confirm_delete_timer;
        let confirm_delete_world_clock = config.confirm_delete_world_clock;
        let confirm_delete_pomodoro = config.confirm_delete_pomodoro;
        let confirm_clear_stopwatch = config.confirm_clear_stopwatch;

        let mut app = AppModel {
            core,
            context_page: ContextPage::default(),
            about,
            nav,
            key_binds: HashMap::new(),
            config,
            config_context,
            use_12h,
            show_shortcuts_dialog: false,
            pending_destructive_action: None,
            confirm_dialog_dont_show_again: false,
            confirm_delete_alarm,
            confirm_delete_timer,
            confirm_delete_world_clock,
            confirm_delete_pomodoro,
            confirm_clear_stopwatch,
            world_clocks,
            stopwatch: stopwatch::StopwatchState::default(),
            alarm,
            timer,
            pomodoro,
            active_timer_id: None,
            active_pomodoro_id: None,
            alarm_audio_stops: HashMap::new(),
        };

        let command = app.update_title();
        (app, command)
    }

    // --- View ---

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            widget::button::custom(widget::text(fl!("view")))
                .padding([4, 12])
                .class(cosmic::theme::Button::MenuRoot)
                .apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![
                    menu::Item::Button(fl!("settings"), None, MenuAction::Settings),
                    menu::Item::Button(fl!("shortcuts"), None, MenuAction::Shortcuts),
                    menu::Item::Button(fl!("about"), None, MenuAction::About),
                ],
            ),
        )]);

        vec![menu_bar.into()]
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => context_drawer::about(
                &self.about,
                |url| Message::LaunchUrl(url.to_string()),
                Message::ToggleContextPage(ContextPage::About),
            ),
            ContextPage::WorldClocksAdd => context_drawer::context_drawer(
                self.world_clocks.sidebar_view().map(Message::WorldClocks),
                Message::ToggleContextPage(ContextPage::WorldClocksAdd),
            )
            .title(fl!("add-clock")),
            ContextPage::StopwatchHistory => context_drawer::context_drawer(
                self.stopwatch.history_view().map(Message::Stopwatch),
                Message::ToggleContextPage(ContextPage::StopwatchHistory),
            )
            .title(fl!("stopwatch-history")),
            ContextPage::AlarmEdit => {
                let title = if self.alarm.editing.as_ref().is_some_and(|e| e.id.is_some()) {
                    fl!("edit-alarm")
                } else {
                    fl!("new-alarm")
                };
                context_drawer::context_drawer(
                    self.alarm.sidebar_view(self.use_12h).map(Message::Alarm),
                    Message::ToggleContextPage(ContextPage::AlarmEdit),
                )
                .title(title)
            }
            ContextPage::TimerAdd => {
                let title = if self.timer.edit_id.is_some() {
                    fl!("edit-timer")
                } else {
                    fl!("add-timer")
                };
                context_drawer::context_drawer(
                    self.timer.sidebar_view().map(Message::Timer),
                    Message::ToggleContextPage(ContextPage::TimerAdd),
                )
                .title(title)
            }
            ContextPage::PomodoroSettings => context_drawer::context_drawer(
                self.pomodoro.settings_view().map(Message::Pomodoro),
                Message::ToggleContextPage(ContextPage::PomodoroSettings),
            )
            .title(fl!("pomodoro-settings")),
            ContextPage::Settings => context_drawer::context_drawer(
                self.settings_view(),
                Message::ToggleContextPage(ContextPage::Settings),
            )
            .title(fl!("settings")),
        })
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let content: Element<_> = match self.nav.active_data::<Page>() {
            Some(Page::WorldClocks) => self
                .world_clocks
                .view(self.use_12h)
                .map(Message::WorldClocks),
            Some(Page::Stopwatch) => self.stopwatch.view().map(Message::Stopwatch),
            Some(Page::Alarm) => self.alarm.view(self.use_12h).map(Message::Alarm),
            Some(Page::Timer) => self.timer.view().map(Message::Timer),
            Some(Page::Pomodoro) => self.pomodoro.view().map(Message::Pomodoro),
            None => widget::text::body(fl!("select-a-view")).into(),
        };

        widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(16)
            .into()
    }

    fn dialog(&self) -> Option<Element<'_, Self::Message>> {
        // Alarm dialog takes priority over shortcuts dialog
        if let Some(ringing) = self.alarm.ringing.first() {
            let aid = ringing.alarm_id;
            let dialog = widget::dialog()
                .title(fl!("alarm-ringing", label = ringing.label.clone()))
                .body(fl!("ringing"))
                .icon(widget::icon::from_name("alarm-symbolic").size(64))
                .primary_action(
                    widget::button::destructive(fl!("dismiss"))
                        .on_press(Message::Alarm(alarm::Message::DismissAlarm(aid))),
                )
                .secondary_action(
                    widget::button::standard(fl!("snooze"))
                        .on_press(Message::Alarm(alarm::Message::SnoozeAlarm(aid))),
                );
            return Some(dialog.into());
        }

        if self.pending_destructive_action.is_some() {
            return Some(self.confirmation_dialog_view());
        }

        if self.show_shortcuts_dialog {
            return Some(self.shortcuts_dialog_view());
        }

        None
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subscriptions = vec![
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(update.config)),
        ];

        subscriptions.push(Subscription::run(tick_subscription));
        subscriptions.push(listen_raw(input_subscription));

        Subscription::batch(subscriptions)
    }

    // --- Update ---

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        let should_save = !matches!(
            message,
            Message::Tick
                | Message::UpdateConfig(_)
                | Message::CloseShortcutsDialog
                | Message::ShowShortcutsDialog
                | Message::CancelDestructiveAction
                | Message::ToggleConfirmDontShowAgain(_)
        );

        match message {
            Message::Tick => {
                self.handle_tick();
            }

            Message::WorldClocks(ref msg) => match msg {
                world_clocks::Message::OpenAddSidebar => {
                    self.context_page = ContextPage::WorldClocksAdd;
                    self.core.window.show_context = true;
                    self.save_state();
                    return widget::text_input::focus(widget::Id::new(
                        "world-clocks-search-input",
                    ));
                }
                world_clocks::Message::RemoveClock(id) => {
                    if self.confirm_delete_world_clock
                        && self.pending_destructive_action.is_none()
                    {
                        let id = *id;
                        self.pending_destructive_action =
                            Some(DestructiveAction::DeleteWorldClock(id));
                        self.confirm_dialog_dont_show_again = false;
                        return Task::none();
                    }
                    self.world_clocks.update(msg.clone());
                }
                _ => {
                    self.world_clocks.update(msg.clone());
                }
            },

            Message::Alarm(ref msg) => match msg {
                alarm::Message::DeleteAlarm(id) => {
                    if self.confirm_delete_alarm && self.pending_destructive_action.is_none() {
                        let id = *id;
                        self.pending_destructive_action = Some(DestructiveAction::DeleteAlarm(id));
                        self.confirm_dialog_dont_show_again = false;
                        return Task::none();
                    }
                    self.alarm.update(msg.clone(), self.use_12h);
                }
                alarm::Message::StartNewAlarm | alarm::Message::StartEditAlarm(_) => {
                    self.alarm.update(msg.clone(), self.use_12h);
                    self.context_page = ContextPage::AlarmEdit;
                    self.core.window.show_context = true;
                    self.save_state();
                    return widget::text_input::focus(widget::Id::new("alarm-label-input"));
                }
                alarm::Message::CancelEdit | alarm::Message::SaveAlarm => {
                    self.alarm.update(msg.clone(), self.use_12h);
                    self.core.window.show_context = false;
                }
                alarm::Message::BrowseCustomSound => {
                    return open_sound_file_dialog(CustomSoundTarget::Alarm);
                }
                alarm::Message::SnoozeAlarm(alarm_id) => {
                    let alarm_id = *alarm_id;
                    self.stop_alarm_audio(alarm_id);
                    self.alarm.update(msg.clone(), self.use_12h);
                }
                alarm::Message::DismissAlarm(alarm_id) => {
                    let alarm_id = *alarm_id;
                    self.stop_alarm_audio(alarm_id);
                    self.alarm.update(msg.clone(), self.use_12h);
                }
                _ => {
                    self.alarm.update(msg.clone(), self.use_12h);
                }
            },

            Message::Timer(ref msg) => match msg {
                timer::Message::DeleteTimer(id) => {
                    if self.confirm_delete_timer && self.pending_destructive_action.is_none() {
                        let id = *id;
                        self.pending_destructive_action = Some(DestructiveAction::DeleteTimer(id));
                        self.confirm_dialog_dont_show_again = false;
                        return Task::none();
                    }
                    self.timer.update(msg.clone());
                }
                timer::Message::StartNew | timer::Message::StartEditTimer(_) => {
                    if let timer::Message::StartEditTimer(id) = msg {
                        self.active_timer_id = Some(*id);
                    }
                    self.timer.update(msg.clone());
                    self.context_page = ContextPage::TimerAdd;
                    self.core.window.show_context = true;
                    self.save_state();
                    return widget::text_input::focus(widget::Id::new("timer-label-input"));
                }
                timer::Message::CancelEdit | timer::Message::SaveTimer => {
                    if matches!(msg, timer::Message::SaveTimer) {
                        // Track the newly saved timer as active
                        if let Some(t) = self.timer.timers.last() {
                            self.active_timer_id = Some(t.id);
                        }
                    }
                    self.timer.update(msg.clone());
                    self.core.window.show_context = false;
                }
                timer::Message::StartTimer(id)
                | timer::Message::PauseTimer(id)
                | timer::Message::ResumeTimer(id) => {
                    self.active_timer_id = Some(*id);
                    self.timer.update(msg.clone());
                }
                timer::Message::BrowseCustomSound => {
                    return open_sound_file_dialog(CustomSoundTarget::Timer);
                }
                timer::Message::Tick => {
                    // Handled above in Message::Tick
                }
                _ => {
                    self.timer.update(msg.clone());
                }
            },

            Message::Stopwatch(ref msg) => match msg {
                stopwatch::Message::OpenHistory => {
                    self.context_page = ContextPage::StopwatchHistory;
                    self.core.window.show_context = true;
                }
                stopwatch::Message::ClearHistory => {
                    if self.confirm_clear_stopwatch && self.pending_destructive_action.is_none() {
                        self.pending_destructive_action =
                            Some(DestructiveAction::ClearStopwatchHistory);
                        self.confirm_dialog_dont_show_again = false;
                        return Task::none();
                    }
                    self.stopwatch.update(msg.clone());
                }
                _ => {
                    self.stopwatch.update(msg.clone());
                }
            },

            Message::Pomodoro(ref msg) => match msg {
                pomodoro::Message::Delete(id) => {
                    if self.confirm_delete_pomodoro && self.pending_destructive_action.is_none() {
                        let id = *id;
                        self.pending_destructive_action =
                            Some(DestructiveAction::DeletePomodoro(id));
                        self.confirm_dialog_dont_show_again = false;
                        return Task::none();
                    }
                    self.pomodoro.update(msg.clone());
                }
                pomodoro::Message::OpenSettings | pomodoro::Message::StartEditPomodoro(_) => {
                    if let pomodoro::Message::StartEditPomodoro(id) = msg {
                        self.active_pomodoro_id = Some(*id);
                    }
                    self.pomodoro.update(msg.clone());
                    self.context_page = ContextPage::PomodoroSettings;
                    self.core.window.show_context = true;
                    self.save_state();
                    return widget::text_input::focus(widget::Id::new("pomodoro-label-input"));
                }
                pomodoro::Message::CancelEditPomodoro | pomodoro::Message::SaveEditPomodoro => {
                    if matches!(msg, pomodoro::Message::SaveEditPomodoro)
                        && let Some(p) = self.pomodoro.timers.last()
                    {
                        self.active_pomodoro_id = Some(p.id);
                    }
                    self.pomodoro.update(msg.clone());
                    self.core.window.show_context = false;
                }
                pomodoro::Message::Start(id)
                | pomodoro::Message::Pause(id)
                | pomodoro::Message::Resume(id)
                | pomodoro::Message::Skip(id) => {
                    self.active_pomodoro_id = Some(*id);
                    self.pomodoro.update(msg.clone());
                }
                pomodoro::Message::BrowseCustomSound => {
                    return open_sound_file_dialog(CustomSoundTarget::Pomodoro);
                }
                pomodoro::Message::Tick => {
                    // Tick handled above
                }
                _ => {
                    self.pomodoro.update(msg.clone());
                }
            },

            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
            }

            Message::UpdateConfig(config) => {
                self.use_12h = config.use_12h;
                self.confirm_delete_alarm = config.confirm_delete_alarm;
                self.confirm_delete_timer = config.confirm_delete_timer;
                self.confirm_delete_world_clock = config.confirm_delete_world_clock;
                self.confirm_delete_pomodoro = config.confirm_delete_pomodoro;
                self.confirm_clear_stopwatch = config.confirm_clear_stopwatch;
                self.config = config;
            }

            Message::CustomSoundSelected(target, path) => match target {
                CustomSoundTarget::Alarm => {
                    self.alarm
                        .update(alarm::Message::EditSound(path), self.use_12h);
                }
                CustomSoundTarget::Timer => {
                    self.timer.update(timer::Message::EditSound(path));
                }
                CustomSoundTarget::Pomodoro => {
                    self.pomodoro.update(pomodoro::Message::EditSound(path));
                }
            },

            Message::SetTimeFormat(use_12h) => {
                self.use_12h = use_12h;
            }

            Message::Quit => {
                std::process::exit(0);
            }

            Message::NavigateNext => {
                let pos = self.nav.position(self.nav.active()).unwrap_or(0);
                let count = self.nav.iter().count() as u16;
                let next = (pos + 1) % count;
                if self.nav.activate_position(next) {
                    self.core.window.show_context = false;
                    return self.update_title();
                }
            }

            Message::NavigatePrev => {
                let pos = self.nav.position(self.nav.active()).unwrap_or(0);
                let count = self.nav.iter().count() as u16;
                let prev = if pos == 0 { count - 1 } else { pos - 1 };
                if self.nav.activate_position(prev) {
                    self.core.window.show_context = false;
                    return self.update_title();
                }
            }

            Message::NavigateTo(pos) => {
                if self.nav.activate_position(pos) {
                    self.core.window.show_context = false;
                    return self.update_title();
                }
            }

            Message::PageShortcutSpace => {
                return self.handle_page_shortcut_space();
            }

            Message::PageShortcutEnter => {
                return self.handle_page_shortcut_enter();
            }

            Message::PageShortcutDelete => {
                return self.handle_page_shortcut_delete();
            }

            Message::PageShortcutCtrlN => {
                return self.handle_page_shortcut_ctrl_n();
            }

            Message::PageShortcutSkip => {
                return self.handle_page_shortcut_skip();
            }

            Message::ShowShortcutsDialog => {
                self.show_shortcuts_dialog = true;
                self.core.window.show_context = false;
            }

            Message::CloseShortcutsDialog => {
                self.show_shortcuts_dialog = false;
            }

            Message::ConfirmDestructiveAction => {
                if self.confirm_dialog_dont_show_again {
                    match &self.pending_destructive_action {
                        Some(DestructiveAction::DeleteAlarm(_)) => {
                            self.confirm_delete_alarm = false;
                        }
                        Some(DestructiveAction::DeleteTimer(_)) => {
                            self.confirm_delete_timer = false;
                        }
                        Some(DestructiveAction::DeleteWorldClock(_)) => {
                            self.confirm_delete_world_clock = false;
                        }
                        Some(DestructiveAction::DeletePomodoro(_)) => {
                            self.confirm_delete_pomodoro = false;
                        }
                        Some(DestructiveAction::ClearStopwatchHistory) => {
                            self.confirm_clear_stopwatch = false;
                        }
                        None => {}
                    }
                }
                match self.pending_destructive_action.take() {
                    Some(DestructiveAction::DeleteAlarm(id)) => {
                        self.alarm.update(alarm::Message::DeleteAlarm(id), self.use_12h);
                    }
                    Some(DestructiveAction::DeleteTimer(id)) => {
                        self.timer.update(timer::Message::DeleteTimer(id));
                    }
                    Some(DestructiveAction::DeleteWorldClock(id)) => {
                        self.world_clocks.update(world_clocks::Message::RemoveClock(id));
                    }
                    Some(DestructiveAction::DeletePomodoro(id)) => {
                        self.pomodoro.update(pomodoro::Message::Delete(id));
                    }
                    Some(DestructiveAction::ClearStopwatchHistory) => {
                        self.stopwatch.update(stopwatch::Message::ClearHistory);
                    }
                    None => {}
                }
                self.confirm_dialog_dont_show_again = false;
            }

            Message::CancelDestructiveAction => {
                self.pending_destructive_action = None;
                self.confirm_dialog_dont_show_again = false;
            }

            Message::ToggleConfirmDontShowAgain(val) => {
                self.confirm_dialog_dont_show_again = val;
            }

            Message::ToggleConfirmationSetting(category, enabled) => {
                match category {
                    ConfirmationCategory::DeleteAlarm => self.confirm_delete_alarm = enabled,
                    ConfirmationCategory::DeleteTimer => self.confirm_delete_timer = enabled,
                    ConfirmationCategory::DeleteWorldClock => {
                        self.confirm_delete_world_clock = enabled;
                    }
                    ConfirmationCategory::DeletePomodoro => {
                        self.confirm_delete_pomodoro = enabled;
                    }
                    ConfirmationCategory::ClearStopwatch => {
                        self.confirm_clear_stopwatch = enabled;
                    }
                }
            }

            Message::LaunchUrl(url) => match open::that_detached(&url) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("failed to open {url:?}: {err}");
                }
            },
        }

        if should_save {
            self.save_state();
        }

        Task::none()
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        self.nav.activate(id);
        self.core.window.show_context = false;
        self.update_title()
    }
}

// --- Private helpers ---

impl AppModel {
    /// Central tick handler: drives stopwatch, timers, pomodoro, and alarm logic
    fn handle_tick(&mut self) {
        // Stopwatch tick
        if self.stopwatch.is_running {
            self.stopwatch.update(stopwatch::Message::Tick);
        }

        // Timer tick + completion notifications
        if self.timer.has_running_timers() {
            let completed = self.timer.update(timer::Message::Tick);
            for (label, sound) in completed {
                audio::send_notification(&fl!("notification-timer-complete"), &label);
                audio::play_sound(&sound);
            }
        }

        // Pomodoro tick + session transition notifications
        if self.pomodoro.is_running() {
            let notifications = self.pomodoro.update(pomodoro::Message::Tick);
            for (msg, sound) in notifications {
                audio::send_notification(&fl!("notification-pomodoro"), &msg);
                audio::play_sound(&sound);
            }
        }

        // Alarm: check for expired ringing (auto-snooze)
        let expired = self.alarm.check_ring_expired();
        for alarm_id in expired {
            self.stop_alarm_audio(alarm_id);
            self.alarm
                .update(alarm::Message::SnoozeAlarm(alarm_id), self.use_12h);
        }

        // Alarm: check snoozed alarms that should re-trigger
        let snoozed_triggers = self.alarm.check_snoozed();
        for info in &snoozed_triggers {
            audio::send_notification(&fl!("notification-alarm-snoozed"), &info.label);
            self.start_alarm_audio(info);
            self.alarm.start_ringing(info);
        }
        if !snoozed_triggers.is_empty() {
            self.save_state();
        }

        // Alarm: check scheduled alarms
        let now = Local::now();
        let triggered =
            self.alarm
                .check_triggers(now.hour() as u8, now.minute() as u8, now.weekday());
        if !triggered.is_empty() {
            for info in &triggered {
                audio::send_notification(&fl!("notification-alarm"), &info.label);
                self.start_alarm_audio(info);
                self.alarm.start_ringing(info);
            }
            self.save_state();
        }
    }

    fn save_state(&self) {
        let Some(ctx) = &self.config_context else {
            return;
        };
        let config = build_config_from_state(
            &self.world_clocks,
            &self.alarm,
            &self.timer,
            &self.pomodoro,
            self.use_12h,
            self.confirm_delete_alarm,
            self.confirm_delete_timer,
            self.confirm_delete_world_clock,
            self.confirm_delete_pomodoro,
            self.confirm_clear_stopwatch,
        );
        if let Err(e) = config.write_entry(ctx) {
            eprintln!("Failed to save config: {:?}", e);
        }
    }

    fn start_alarm_audio(&mut self, info: &alarm::AlarmTriggerInfo) {
        let stop = Arc::new(AtomicBool::new(false));
        self.alarm_audio_stops.insert(info.alarm_id, stop.clone());
        let sound = info.sound.clone();
        let ring_secs = info.ring_secs;
        std::thread::spawn(move || {
            if let Err(e) = audio::play_alarm_sound_loop(&sound, ring_secs, stop) {
                eprintln!("Alarm audio error: {}", e);
            }
        });
    }

    fn stop_alarm_audio(&mut self, alarm_id: u32) {
        if let Some(stop) = self.alarm_audio_stops.remove(&alarm_id) {
            stop.store(true, Ordering::Relaxed);
        }
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let spacing = 12;
        let mut col = widget::column::with_capacity(12).spacing(spacing);

        col = col.push(widget::text::body(fl!("time-format")));

        let btn_24h = if self.use_12h {
            widget::button::standard(fl!("time-format-24h")).on_press(Message::SetTimeFormat(false))
        } else {
            widget::button::suggested(fl!("time-format-24h"))
                .on_press(Message::SetTimeFormat(false))
        };
        let btn_12h = if self.use_12h {
            widget::button::suggested(fl!("time-format-12h")).on_press(Message::SetTimeFormat(true))
        } else {
            widget::button::standard(fl!("time-format-12h")).on_press(Message::SetTimeFormat(true))
        };

        let row = widget::row::with_capacity(2)
            .spacing(8)
            .push(btn_24h)
            .push(btn_12h);
        col = col.push(row);

        col = col.push(widget::divider::horizontal::default());
        col = col.push(widget::text::title4(fl!("settings-section-confirmation-dialogs")));

        col = col.push(
            widget::checkbox(self.confirm_delete_alarm)
                .label(fl!("settings-confirm-delete-alarm"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::DeleteAlarm, v)
                }),
        );
        col = col.push(
            widget::checkbox(self.confirm_delete_timer)
                .label(fl!("settings-confirm-delete-timer"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::DeleteTimer, v)
                }),
        );
        col = col.push(
            widget::checkbox(self.confirm_delete_world_clock)
                .label(fl!("settings-confirm-delete-world-clock"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::DeleteWorldClock, v)
                }),
        );
        col = col.push(
            widget::checkbox(self.confirm_delete_pomodoro)
                .label(fl!("settings-confirm-delete-pomodoro"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::DeletePomodoro, v)
                }),
        );
        col = col.push(
            widget::checkbox(self.confirm_clear_stopwatch)
                .label(fl!("settings-confirm-clear-stopwatch"))
                .on_toggle(|v| {
                    Message::ToggleConfirmationSetting(ConfirmationCategory::ClearStopwatch, v)
                }),
        );

        col.into()
    }

    fn active_timer(&self) -> Option<&timer::TimerEntry> {
        self.active_timer_id
            .and_then(|id| self.timer.timers.iter().find(|t| t.id == id))
            .or_else(|| self.timer.timers.first())
    }

    fn active_pomodoro(&self) -> Option<&pomodoro::PomodoroTimer> {
        self.active_pomodoro_id
            .and_then(|id| self.pomodoro.timers.iter().find(|p| p.id == id))
            .or_else(|| self.pomodoro.timers.first())
    }

    fn handle_page_shortcut_space(&mut self) -> Task<cosmic::Action<Message>> {
        match self.nav.active_data::<Page>() {
            Some(Page::Stopwatch) => {
                if self.stopwatch.is_running {
                    self.stopwatch.update(stopwatch::Message::Stop);
                } else {
                    self.stopwatch.update(stopwatch::Message::Start);
                }
                self.save_state();
            }
            Some(Page::Timer) => {
                if let Some(t) = self.active_timer() {
                    let id = t.id;
                    let msg = if t.is_running {
                        timer::Message::PauseTimer(id)
                    } else if t.remaining < t.initial_duration {
                        timer::Message::ResumeTimer(id)
                    } else {
                        timer::Message::StartTimer(id)
                    };
                    self.active_timer_id = Some(id);
                    self.timer.update(msg);
                    self.save_state();
                }
            }
            Some(Page::Pomodoro) => {
                if let Some(p) = self.active_pomodoro() {
                    let id = p.id;
                    let msg = if p.is_running {
                        pomodoro::Message::Pause(id)
                    } else if p.remaining < p.started_remaining {
                        pomodoro::Message::Resume(id)
                    } else {
                        pomodoro::Message::Start(id)
                    };
                    self.active_pomodoro_id = Some(id);
                    self.pomodoro.update(msg);
                    self.save_state();
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn handle_page_shortcut_enter(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(Page::Stopwatch) = self.nav.active_data::<Page>()
            && self.stopwatch.is_running
        {
            self.stopwatch.update(stopwatch::Message::Lap);
            self.save_state();
        }
        Task::none()
    }

    fn handle_page_shortcut_delete(&mut self) -> Task<cosmic::Action<Message>> {
        match self.nav.active_data::<Page>() {
            Some(Page::Stopwatch) => {
                if !self.stopwatch.is_running
                    && self.stopwatch.elapsed > std::time::Duration::ZERO
                {
                    self.stopwatch.update(stopwatch::Message::Reset);
                    self.save_state();
                }
            }
            Some(Page::Timer) => {
                if let Some(t) = self.active_timer() {
                    let id = t.id;
                    if !t.is_running && t.remaining < t.initial_duration {
                        self.timer.update(timer::Message::ResetTimer(id));
                        self.save_state();
                    }
                }
            }
            Some(Page::Pomodoro) => {
                if let Some(p) = self.active_pomodoro() {
                    let id = p.id;
                    if !p.is_running {
                        self.pomodoro.update(pomodoro::Message::Reset(id));
                        self.save_state();
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn handle_page_shortcut_ctrl_n(&mut self) -> Task<cosmic::Action<Message>> {
        match self.nav.active_data::<Page>() {
            Some(Page::WorldClocks) => {
                self.context_page = ContextPage::WorldClocksAdd;
                self.core.window.show_context = true;
                self.save_state();
                return widget::text_input::focus(widget::Id::new(
                    "world-clocks-search-input",
                ));
            }
            Some(Page::Alarm) => {
                self.alarm.update(alarm::Message::StartNewAlarm, self.use_12h);
                self.context_page = ContextPage::AlarmEdit;
                self.core.window.show_context = true;
                self.save_state();
                return widget::text_input::focus(widget::Id::new("alarm-label-input"));
            }
            Some(Page::Timer) => {
                self.timer.update(timer::Message::StartNew);
                self.context_page = ContextPage::TimerAdd;
                self.core.window.show_context = true;
                self.save_state();
                return widget::text_input::focus(widget::Id::new("timer-label-input"));
            }
            Some(Page::Pomodoro) => {
                self.pomodoro.update(pomodoro::Message::AddTimer);
                self.save_state();
            }
            _ => {}
        }
        Task::none()
    }

    fn handle_page_shortcut_skip(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(Page::Pomodoro) = self.nav.active_data::<Page>()
            && let Some(p) = self.active_pomodoro()
        {
            let id = p.id;
            if p.is_running
                && matches!(
                    p.session_type,
                    pomodoro::SessionType::ShortBreak | pomodoro::SessionType::LongBreak
                )
            {
                self.active_pomodoro_id = Some(id);
                self.pomodoro.update(pomodoro::Message::Skip(id));
                self.save_state();
            }
        }
        Task::none()
    }

    fn shortcuts_dialog_view(&self) -> Element<'_, Message> {
        let spacing = 10;
        let mut col = widget::column::with_capacity(26).spacing(spacing);

        // Global shortcuts
        col = col.push(widget::text::title4(fl!("shortcuts-global")));
        col = col.push(Self::shortcut_row(fl!("shortcuts-quit"), &["Ctrl", "Q"]));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-next-tab"),
            &["Ctrl", "↓"],
        ));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-prev-tab"),
            &["Ctrl", "↑"],
        ));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-show-shortcuts"),
            &["Ctrl", "?"],
        ));

        col = col.push(widget::divider::horizontal::default());

        // Tab shortcuts
        col = col.push(widget::text::title4(fl!("shortcuts-tabs")));
        col = col.push(Self::shortcut_row(fl!("nav-world-clocks"), &["Alt", "1"]));
        col = col.push(Self::shortcut_row(fl!("nav-stopwatch"), &["Alt", "2"]));
        col = col.push(Self::shortcut_row(fl!("nav-alarm"), &["Alt", "3"]));
        col = col.push(Self::shortcut_row(fl!("nav-timer"), &["Alt", "4"]));
        col = col.push(Self::shortcut_row(fl!("nav-pomodoro"), &["Alt", "5"]));

        col = col.push(widget::divider::horizontal::default());

        // Page shortcuts
        col = col.push(widget::text::title4(fl!("shortcuts-page")));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-start-pause"),
            &["Space"],
        ));
        col = col.push(Self::shortcut_row(fl!("shortcuts-lap"), &["Enter"]));
        col = col.push(Self::shortcut_row(fl!("shortcuts-reset"), &["Delete"]));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-new-item"),
            &["Ctrl", "N"],
        ));
        col = col.push(Self::shortcut_row(
            fl!("shortcuts-skip-break"),
            &["Ctrl", "S"],
        ));

        let dialog = widget::dialog()
            .title(fl!("shortcuts"))
            .body(fl!("shortcuts-description"))
            .control(col)
            .primary_action(
                widget::button::standard(fl!("shortcuts-close"))
                    .on_press(Message::CloseShortcutsDialog),
            );

        dialog.into()
    }

    fn confirmation_dialog_view(&self) -> Element<'_, Message> {
        let (title, body, confirm_label) = match &self.pending_destructive_action {
            Some(DestructiveAction::DeleteAlarm(_)) => (
                fl!("confirm-delete-alarm-title"),
                fl!("confirm-delete-alarm-body"),
                fl!("confirm-button-delete"),
            ),
            Some(DestructiveAction::DeleteTimer(_)) => (
                fl!("confirm-delete-timer-title"),
                fl!("confirm-delete-timer-body"),
                fl!("confirm-button-delete"),
            ),
            Some(DestructiveAction::DeleteWorldClock(_)) => (
                fl!("confirm-delete-world-clock-title"),
                fl!("confirm-delete-world-clock-body"),
                fl!("confirm-button-delete"),
            ),
            Some(DestructiveAction::DeletePomodoro(_)) => (
                fl!("confirm-delete-pomodoro-title"),
                fl!("confirm-delete-pomodoro-body"),
                fl!("confirm-button-delete"),
            ),
            Some(DestructiveAction::ClearStopwatchHistory) => (
                fl!("confirm-clear-stopwatch-title"),
                fl!("confirm-clear-stopwatch-body"),
                fl!("confirm-button-clear"),
            ),
            None => return widget::text::body("").into(),
        };

        let dont_show = widget::checkbox(self.confirm_dialog_dont_show_again)
            .label(fl!("confirm-dont-show-again"))
            .on_toggle(Message::ToggleConfirmDontShowAgain);

        widget::dialog()
            .title(title)
            .body(body)
            .control(dont_show)
            .primary_action(
                widget::button::destructive(confirm_label)
                    .on_press(Message::ConfirmDestructiveAction),
            )
            .secondary_action(
                widget::button::standard(fl!("confirm-button-cancel"))
                    .on_press(Message::CancelDestructiveAction),
            )
            .into()
    }

    fn shortcut_row<'a>(action: String, keys: &'a [&'a str]) -> Element<'a, Message> {
        use cosmic::iced::widget::container as iced_container;
        use cosmic::iced_core::{Background, Border};

        let keys_row = keys.iter().fold(
            widget::row::with_capacity(keys.len() * 2)
                .spacing(4)
                .align_y(cosmic::iced::Alignment::Center),
            |row, key| {
                row.push(
                    widget::container(
                        widget::text::body(key.to_string())
                            .size(13.0)
                            .align_x(cosmic::iced::Alignment::Center),
                    )
                    .padding([2, 8])
                    .style(|theme: &cosmic::Theme| {
                        let cosmic = theme.cosmic();
                        iced_container::Style {
                            background: Some(Background::Color(
                                cosmic.background.component.hover.into(),
                            )),
                            border: Border {
                                color: cosmic.background.component.divider.into(),
                                width: 1.0,
                                radius: cosmic.corner_radii.radius_xs.into(),
                            },
                            ..Default::default()
                        }
                    }),
                )
            },
        );

        widget::row::with_capacity(2)
            .push(widget::text::body(action).width(Length::FillPortion(3)))
            .push(
                widget::container(keys_row)
                    .width(Length::FillPortion(2))
                    .align_x(cosmic::iced::Alignment::End),
            )
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    }

    fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let mut window_title = fl!("app-title");

        if let Some(page) = self.nav.text(self.nav.active()) {
            window_title.push_str(" — ");
            window_title.push_str(page);
        }

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }
}

// --- Subscriptions ---

fn tick_subscription() -> impl futures_util::Stream<Item = Message> {
    use futures_util::SinkExt;
    iced_futures::stream::channel(1, async |mut emitter| {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            _ = emitter.send(Message::Tick).await;
        }
    })
}

fn input_subscription(
    event: cosmic::iced::Event,
    status: cosmic::iced::event::Status,
    _window: cosmic::iced::window::Id,
) -> Option<Message> {
    // Only handle ignored events (not consumed by widgets like text inputs)
    if status != cosmic::iced::event::Status::Ignored {
        return None;
    }

    let cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
        key, modifiers, ..
    }) = event
    else {
        return None;
    };

    let ctrl = modifiers.control();
    let alt = modifiers.alt();

    // For non-alphabetic character keys, Shift is inherent to producing the
    // character (e.g. Shift+/ → ?), so strip it to avoid mismatches when a
    // binding is defined for the shifted character without an explicit Shift
    // modifier.
    let shift = match key.as_ref() {
        Key::Character(c) if c.chars().all(|ch| !ch.is_ascii_alphabetic()) => false,
        _ => modifiers.shift(),
    };

    match key.as_ref() {
        // Ctrl+Q → Quit
        Key::Character("q") if ctrl && !alt && !shift => Some(Message::Quit),

        // Ctrl+PageDown → Next tab
        Key::Named(Named::PageDown) if ctrl && !alt && !shift => Some(Message::NavigateNext),

        // Ctrl+PageUp → Previous tab
        Key::Named(Named::PageUp) if ctrl && !alt && !shift => Some(Message::NavigatePrev),

        // Alt+1..5 → Direct tab navigation
        Key::Character("1") if alt && !ctrl && !shift => Some(Message::NavigateTo(0)),
        Key::Character("2") if alt && !ctrl && !shift => Some(Message::NavigateTo(1)),
        Key::Character("3") if alt && !ctrl && !shift => Some(Message::NavigateTo(2)),
        Key::Character("4") if alt && !ctrl && !shift => Some(Message::NavigateTo(3)),
        Key::Character("5") if alt && !ctrl && !shift => Some(Message::NavigateTo(4)),

        // Ctrl+N → New item (page-scoped)
        Key::Character("n") if ctrl && !alt && !shift => Some(Message::PageShortcutCtrlN),

        // Ctrl+? (Ctrl+Shift+/) → Show shortcuts dialog
        Key::Character("?") if ctrl && !alt => Some(Message::ShowShortcutsDialog),
        // Fallback: some platforms report the physical key "/" instead of "?"
        Key::Character("/") if ctrl && !alt && modifiers.shift() => {
            Some(Message::ShowShortcutsDialog)
        }

        // Ctrl+S → Skip break (Pomodoro)
        Key::Character("s") if ctrl && !alt && !shift => Some(Message::PageShortcutSkip),

        // Space → Start/Pause (page-scoped)
        Key::Character(" ") if !ctrl && !alt && !shift => Some(Message::PageShortcutSpace),

        // Enter → Lap (page-scoped)
        Key::Named(Named::Enter) if !ctrl && !alt && !shift => Some(Message::PageShortcutEnter),

        // Delete / Backspace → Reset (page-scoped)
        Key::Named(Named::Delete | Named::Backspace) if !ctrl && !alt && !shift => {
            Some(Message::PageShortcutDelete)
        }

        // Escape → Close shortcuts dialog
        Key::Named(Named::Escape) => Some(Message::CloseShortcutsDialog),

        _ => None,
    }
}

fn open_sound_file_dialog(target: CustomSoundTarget) -> Task<cosmic::Action<Message>> {
    cosmic::task::future(async move {
        let dialog = cosmic::dialog::file_chooser::open::Dialog::new()
            .title(crate::fl!("choose-sound-file"));

        match dialog.open_file().await {
            Ok(response) => {
                let path = response.url().path().to_string();
                cosmic::Action::App(Message::CustomSoundSelected(target, path))
            }
            Err(_) => cosmic::Action::App(Message::Tick),
        }
    })
}

// --- Persistence: build Config from runtime state ---

#[allow(clippy::too_many_arguments)]
fn build_config_from_state(
    wc: &world_clocks::WorldClocksState,
    al: &alarm::AlarmState,
    ti: &timer::TimerState,
    po: &pomodoro::PomodoroState,
    use_12h: bool,
    confirm_delete_alarm: bool,
    confirm_delete_timer: bool,
    confirm_delete_world_clock: bool,
    confirm_delete_pomodoro: bool,
    confirm_clear_stopwatch: bool,
) -> Config {
    let world_clocks = wc
        .clocks
        .iter()
        .map(|c| SavedClock {
            timezone: c.timezone,
            city_name: c.city_name.clone(),
            is_local: c.is_local,
        })
        .collect();

    let alarms = al
        .alarms
        .iter()
        .map(|a| {
            let repeat_mode = match &a.repeat_mode {
                alarm::RepeatMode::Once => SavedRepeatMode::Once,
                alarm::RepeatMode::EveryDay => SavedRepeatMode::EveryDay,
                alarm::RepeatMode::Custom(days) => SavedRepeatMode::Custom(
                    days.iter().map(|d| d.short_name().to_string()).collect(),
                ),
            };
            SavedAlarm {
                hour: a.hour,
                minute: a.minute,
                label: a.label.clone(),
                is_enabled: a.is_enabled,
                repeat_mode,
                sound: a.sound.clone(),
                snooze_minutes: a.snooze_minutes,
                ring_minutes: a.ring_minutes,
            }
        })
        .collect();

    let timers = ti
        .timers
        .iter()
        .map(|t| SavedTimer {
            label: t.label.clone(),
            duration_secs: t.initial_duration.as_secs(),
            repeat_enabled: t.repeat_enabled,
            repeat_count: t.repeat_count,
            sound: t.sound.clone(),
        })
        .collect();

    let pomodoros = po
        .timers
        .iter()
        .map(|p| SavedPomodoro {
            label: p.label.clone(),
            work_minutes: p.work_minutes,
            short_break_minutes: p.short_break_minutes,
            long_break_minutes: p.long_break_minutes,
            sound: p.sound.clone(),
        })
        .collect();

    let pomodoro_defaults = PomodoroDefaults {
        work_minutes: po.default_work_minutes,
        short_break_minutes: po.default_short_break_minutes,
        long_break_minutes: po.default_long_break_minutes,
    };

    Config {
        world_clocks,
        alarms,
        timers,
        pomodoros,
        pomodoro_defaults,
        use_12h,
        confirm_delete_alarm,
        confirm_delete_timer,
        confirm_delete_world_clock,
        confirm_delete_pomodoro,
        confirm_clear_stopwatch,
    }
}

// --- Persistence: restore runtime state from Config ---

fn restore_world_clocks(config: &Config) -> world_clocks::WorldClocksState {
    if config.world_clocks.is_empty() {
        return world_clocks::WorldClocksState::default();
    }

    let clocks: Vec<world_clocks::ClockEntry> = config
        .world_clocks
        .iter()
        .enumerate()
        .map(|(i, c)| world_clocks::ClockEntry {
            id: i as u32,
            timezone: c.timezone,
            city_name: c.city_name.clone(),
            is_local: c.is_local,
        })
        .collect();

    let local_tz = clocks
        .iter()
        .find(|c| c.is_local)
        .map(|c| c.timezone)
        .unwrap_or_else(|| {
            iana_time_zone::get_timezone()
                .ok()
                .and_then(|tz_str| tz_str.parse().ok())
                .unwrap_or(chrono_tz::UTC)
        });

    let next_id = clocks.len() as u32;

    world_clocks::WorldClocksState {
        local_timezone: local_tz,
        clocks,
        next_id,
        search_text: String::new(),
        filtered_timezones: Vec::new(),
    }
}

fn restore_alarms(config: &Config) -> alarm::AlarmState {
    let alarms: Vec<alarm::AlarmEntry> = config
        .alarms
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let repeat_mode = match &a.repeat_mode {
                SavedRepeatMode::Once => alarm::RepeatMode::Once,
                SavedRepeatMode::EveryDay => alarm::RepeatMode::EveryDay,
                SavedRepeatMode::Custom(days) => {
                    let parsed: Vec<alarm::DayOfWeek> = days
                        .iter()
                        .filter_map(|d| match d.as_str() {
                            "Mon" => Some(alarm::DayOfWeek::Monday),
                            "Tue" => Some(alarm::DayOfWeek::Tuesday),
                            "Wed" => Some(alarm::DayOfWeek::Wednesday),
                            "Thu" => Some(alarm::DayOfWeek::Thursday),
                            "Fri" => Some(alarm::DayOfWeek::Friday),
                            "Sat" => Some(alarm::DayOfWeek::Saturday),
                            "Sun" => Some(alarm::DayOfWeek::Sunday),
                            _ => None,
                        })
                        .collect();
                    if parsed.is_empty() {
                        alarm::RepeatMode::Once
                    } else {
                        alarm::RepeatMode::Custom(parsed)
                    }
                }
            };
            // Migrate "Default" sound to "Bell"
            let sound = if a.sound == "Default" {
                "Bell".to_string()
            } else {
                a.sound.clone()
            };
            alarm::AlarmEntry {
                id: (i + 1) as u32,
                hour: a.hour,
                minute: a.minute,
                label: a.label.clone(),
                is_enabled: a.is_enabled,
                repeat_mode,
                sound,
                snooze_minutes: a.snooze_minutes,
                ring_minutes: a.ring_minutes,
            }
        })
        .collect();

    let next_id = alarms.len() as u32 + 1;

    alarm::AlarmState {
        alarms,
        next_id,
        editing: None,
        last_triggered_minute: None,
        ringing: Vec::new(),
        snoozed: Vec::new(),
    }
}

fn restore_timers(config: &Config) -> timer::TimerState {
    let timers: Vec<timer::TimerEntry> = config
        .timers
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let dur = Duration::from_secs(t.duration_secs);
            // Migrate "Default" sound to "Bell"
            let sound = if t.sound == "Default" {
                "Bell".to_string()
            } else {
                t.sound.clone()
            };
            timer::TimerEntry {
                id: (i + 1) as u32,
                label: t.label.clone(),
                initial_duration: dur,
                remaining: dur,
                is_running: false,
                start_instant: None,
                started_remaining: dur,
                repeat_enabled: t.repeat_enabled,
                repeat_count: t.repeat_count,
                completed_count: 0,
                sound,
            }
        })
        .collect();

    let next_id = timers.len() as u32 + 1;

    timer::TimerState {
        timers,
        next_id,
        editing: false,
        edit_id: None,
        edit_hours: 0,
        edit_minutes: 5,
        edit_seconds: 0,
        edit_label: String::new(),
        edit_repeat: false,
        edit_repeat_count: 1,
        edit_sound: "Bell".to_string(),
    }
}

fn restore_pomodoros(config: &Config) -> pomodoro::PomodoroState {
    let mut state = pomodoro::PomodoroState {
        default_work_minutes: config.pomodoro_defaults.work_minutes,
        default_short_break_minutes: config.pomodoro_defaults.short_break_minutes,
        default_long_break_minutes: config.pomodoro_defaults.long_break_minutes,
        ..Default::default()
    };

    if !config.pomodoros.is_empty() {
        state.timers.clear();
        for (i, p) in config.pomodoros.iter().enumerate() {
            let mut timer = pomodoro::PomodoroTimer::from_config(
                i as u32,
                p.label.clone(),
                p.work_minutes,
                p.short_break_minutes,
                p.long_break_minutes,
            );
            // Migrate "Default" sound to "Bell"
            timer.sound = if p.sound == "Default" {
                "Bell".to_string()
            } else {
                p.sound.clone()
            };
            state.timers.push(timer);
        }
        state.next_id = config.pomodoros.len() as u32;
    }

    state
}
