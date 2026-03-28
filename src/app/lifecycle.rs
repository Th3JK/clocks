// SPDX-License-Identifier: MIT
//
// Implements the `cosmic::Application` trait for `AppModel`.

use super::persistence::{restore_alarms, restore_pomodoros, restore_timers, restore_world_clocks};
use super::subscriptions::{input_subscription, open_sound_file_dialog, tick_subscription};
use super::{
    AppModel, ConfirmationCategory, CustomSoundTarget, DestructiveAction, MenuAction, Message,
    APP_ICON, REPOSITORY,
};
use cosmic::widget::toaster;
use crate::config::Config;
use crate::fl;
use crate::pages::{ContextPage, Page, alarm, pomodoro, stopwatch, timer, world_clocks};
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::Length;
use cosmic::iced::Subscription;
use cosmic::iced_futures::event::listen_raw;
use cosmic::widget::{self, about::About, icon, menu, nav_bar};
use cosmic::prelude::*;
use std::collections::HashMap;

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
        let auto_sort_alarms = config.auto_sort_alarms;
        let auto_sort_world_clocks = config.auto_sort_world_clocks;

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
            auto_sort_alarms,
            auto_sort_world_clocks,
            world_clocks,
            stopwatch: stopwatch::StopwatchState::default(),
            alarm,
            timer,
            pomodoro,
            active_timer_id: None,
            active_pomodoro_id: None,
            alarm_audio_stops: HashMap::new(),
            toasts: toaster::Toasts::new(Message::CloseToast),
        };

        if app.auto_sort_alarms {
            app.sort_alarms();
        }
        if app.auto_sort_world_clocks {
            app.sort_world_clocks();
        }

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
                .view(self.use_12h, self.auto_sort_world_clocks)
                .map(Message::WorldClocks),
            Some(Page::Stopwatch) => self.stopwatch.view().map(Message::Stopwatch),
            Some(Page::Alarm) => self.alarm.view(self.use_12h, self.auto_sort_alarms).map(Message::Alarm),
            Some(Page::Timer) => self.timer.view().map(Message::Timer),
            Some(Page::Pomodoro) => self.pomodoro.view().map(Message::Pomodoro),
            None => widget::text::body(fl!("select-a-view")).into(),
        };

        let page = widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(16);

        toaster::toaster(&self.toasts, page).into()
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
                | Message::CloseToast(_)
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
                alarm::Message::ToggleAlarm(id) => {
                    let id = *id;
                    self.alarm.update(msg.clone(), self.use_12h);
                    // Show toast when alarm is enabled
                    if let Some(alarm) = self.alarm.alarms.iter().find(|a| a.id == id) {
                        if alarm.is_enabled {
                            let alarm = alarm.clone();
                            let task = self.push_alarm_toast(&alarm);
                            self.save_state();
                            return task;
                        }
                    }
                }
                alarm::Message::DeleteAlarm(id) => {
                    if self.confirm_delete_alarm && self.pending_destructive_action.is_none() {
                        let id = *id;
                        self.pending_destructive_action = Some(DestructiveAction::DeleteAlarm(id));
                        self.confirm_dialog_dont_show_again = false;
                        return Task::none();
                    }
                    self.alarm.update(msg.clone(), self.use_12h);
                    // Close the edit drawer if it was open
                    if self.context_page == ContextPage::AlarmEdit {
                        self.alarm.editing = None;
                        self.core.window.show_context = false;
                    }
                }
                alarm::Message::StartNewAlarm | alarm::Message::StartEditAlarm(_) => {
                    self.alarm.update(msg.clone(), self.use_12h);
                    self.context_page = ContextPage::AlarmEdit;
                    self.core.window.show_context = true;
                    self.save_state();
                    return widget::text_input::focus(widget::Id::new("alarm-label-input"));
                }
                alarm::Message::CancelEdit => {
                    self.alarm.update(msg.clone(), self.use_12h);
                    self.core.window.show_context = false;
                }
                alarm::Message::SaveAlarm => {
                    self.alarm.update(msg.clone(), self.use_12h);
                    self.core.window.show_context = false;
                    // Show toast for newly created alarm (enabled by default)
                    if let Some(alarm) = self.alarm.alarms.last() {
                        if alarm.is_enabled {
                            let alarm = alarm.clone();
                            let task = self.push_alarm_toast(&alarm);
                            self.save_state();
                            return task;
                        }
                    }
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
                stopwatch::Message::ResumeFromHistory(_) => {
                    self.stopwatch.update(msg.clone());
                    self.core.window.show_context = false;
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
                self.auto_sort_alarms = config.auto_sort_alarms;
                self.auto_sort_world_clocks = config.auto_sort_world_clocks;
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
                        if self.context_page == ContextPage::AlarmEdit {
                            self.alarm.editing = None;
                            self.core.window.show_context = false;
                        }
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

            Message::CloseToast(id) => {
                self.toasts.remove(id);
            }

            Message::SetAutoSortAlarms(enabled) => {
                self.auto_sort_alarms = enabled;
                if enabled {
                    self.sort_alarms();
                }
            }

            Message::SetAutoSortWorldClocks(enabled) => {
                self.auto_sort_world_clocks = enabled;
                if enabled {
                    self.sort_world_clocks();
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
