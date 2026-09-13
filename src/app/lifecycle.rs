// SPDX-License-Identifier: MIT
//
// Implements the `cosmic::Application` trait for `AppModel`.

use super::persistence::{
    restore_alarms, restore_chess, restore_countdowns, restore_nav, restore_pomodoros, restore_stopwatch_history, restore_timers,
    restore_workouts, restore_world_clocks,
};
use super::subscriptions::{
    input_subscription, open_sound_file_dialog, save_csv_dialog, tick_subscription,
};
use super::{
    AppModel, ConfirmationCategory, CustomSoundTarget, DestructiveAction, MenuAction, Message,
    APP_ICON, REPOSITORY,
};
use cosmic::widget::toaster;
use crate::config::Config;
use crate::fl;
use crate::pages::{
    ContextPage, Page, alarm, chess, countdown, pomodoro, stopwatch, timer, workout, world_clocks,
};
use cosmic::app::context_drawer;
use cosmic_config::CosmicConfigEntry;
use cosmic::iced::Length;
use cosmic::iced::Subscription;
use cosmic::iced::event::listen_raw;
use cosmic::widget::{self, about::About, icon, menu, nav_bar};
use cosmic::prelude::*;
use std::collections::HashMap;

// --- Application trait (View + Update lifecycle) ---

/// Send a command to the daemon without waiting for it.
///
/// Blocking zbus on a detached thread rather than a `Task`: the GUI does not
/// need the result, because the state change comes back through the runtime
/// watcher. Doing it inline would block the event loop on a D-Bus round-trip.
fn daemon_call(call: impl FnOnce() -> Result<(), zbus::Error> + Send + 'static) {
    std::thread::spawn(move || {
        if let Err(e) = call() {
            eprintln!("clocks: could not reach the daemon: {e}");
        }
    });
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = crate::flags::Flags;
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
        flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Populated by `rebuild_nav` once the stored order is known.
        let nav = nav_bar::Model::default();

        let about = About::default()
            .name(fl!("app-title"))
            .icon(widget::icon::from_svg_bytes(APP_ICON))
            .version(env!("CARGO_PKG_VERSION"))
            // `links` replaces rather than appends, so both entries go in one
            // call. The tuple is (label, url) — the contributor setters are not
            // an option here, as they rewrite their second element as `mailto:`.
            .links([
                (fl!("repository"), REPOSITORY.to_string()),
                (fl!("report-issue"), format!("{REPOSITORY}/issues")),
            ])
            .license(env!("CARGO_PKG_LICENSE"))
            // Without a URL the about widget still wires `on_press`, so the
            // license row was clickable and fired `LaunchUrl("")`.
            .license_url(format!("{REPOSITORY}/blob/main/LICENSE"))
            .comments(env!("CARGO_PKG_DESCRIPTION"));

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
        let stopwatch = restore_stopwatch_history(&config);
        let chess = restore_chess(&config);
        let workout = restore_workouts(&config);
        let countdown = restore_countdowns(&config);
        let (nav_order, nav_hidden) = restore_nav(&config);

        let time_format = config.time_format;
        let use_12h = time_format.use_12h();
        let confirm_delete_alarm = config.confirm_delete_alarm;
        let confirm_delete_timer = config.confirm_delete_timer;
        let confirm_delete_world_clock = config.confirm_delete_world_clock;
        let confirm_delete_pomodoro = config.confirm_delete_pomodoro;
        let confirm_clear_stopwatch = config.confirm_clear_stopwatch;
        let auto_sort_alarms = config.auto_sort_alarms;
        let auto_sort_world_clocks = config.auto_sort_world_clocks;
        let auto_clear_stopwatch_history = config.auto_clear_stopwatch_history;

        let mut app = AppModel {
            core,
            context_page: ContextPage::default(),
            about,
            nav,
            key_binds: key_binds(),
            config,
            config_context,
            time_format,
            use_12h,
            show_shortcuts_dialog: false,
            nav_order,
            nav_hidden,
            show_settings: false,
            nav_dragging: None,
            nav_pre_drag: Vec::new(),
            runtime: crate::runtime::RuntimeState::default(),
            show_palette: false,
            palette_input: String::new(),
            pending_destructive_action: None,
            confirm_dialog_dont_show_again: false,
            confirm_delete_alarm,
            confirm_delete_timer,
            confirm_delete_world_clock,
            confirm_delete_pomodoro,
            confirm_clear_stopwatch,
            auto_sort_alarms,
            auto_sort_world_clocks,
            auto_clear_stopwatch_history,
            world_clocks,
            stopwatch,
            alarm,
            timer,
            pomodoro,
            chess,
            workout,
            countdown,
            active_timer_id: None,
            active_pomodoro_id: None,
            toasts: toaster::Toasts::new(Message::CloseToast),
        };

        app.rebuild_nav();

        // Bring the daemon up if it is not already running: this call is what
        // D-Bus activation hangs off, so alarms work while the app is open even
        // when autostart was never granted.
        std::thread::spawn(|| {
            if let Err(e) = crate::ipc::reload() {
                eprintln!("clocks: background daemon unavailable: {e}");
            }
        });

        // Launched with a page to show -- e.g. by clicking an alarm
        // notification while the app was closed.
        if let Some(page) = flags.page {
            app.activate_page(page);
        }

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
            // The system glyph rather than a bundled one: this sits next to the
            // nav-bar toggle, and only the real icon matches its weight.
            widget::button::custom(icon::from_name("open-menu-symbolic").size(16).icon())
                .padding([4, 12])
                .class(cosmic::theme::Button::MenuRoot)
                .apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![
                    menu::Item::Button(fl!("palette-title"), None, MenuAction::QuickAction),
                    menu::Item::Button(fl!("settings"), None, MenuAction::Settings),
                    menu::Item::Button(fl!("shortcuts"), None, MenuAction::Shortcuts),
                    menu::Item::Button(fl!("about"), None, MenuAction::About),
                ],
            ),
        )]);

        // `MenuBar` defaults to `ItemWidth::Uniform(150)`, which ignores each
        // tree's own width outright -- that 150 is the cramped dropdown. Setting
        // it here is what actually widens the menu; `MenuTree::width` is only
        // consulted under `ItemWidth::Static`.
        let menu_bar = menu_bar
            .item_width(menu::ItemWidth::Uniform(260))
            .item_height(menu::ItemHeight::Uniform(36));

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
            ContextPage::PomodoroSettings => {
                let title = if self.pomodoro.editing_id.is_some() {
                    fl!("edit-pomodoro")
                } else {
                    fl!("new-pomodoro")
                };
                context_drawer::context_drawer(
                    self.pomodoro.settings_view().map(Message::Pomodoro),
                    Message::ToggleContextPage(ContextPage::PomodoroSettings),
                )
                .title(title)
            }
            ContextPage::ChessSettings => context_drawer::context_drawer(
                self.chess.settings_view().map(Message::Chess),
                Message::ToggleContextPage(ContextPage::ChessSettings),
            )
            .title(fl!("chess-settings")),
            ContextPage::WorkoutEdit => {
                let title = if self.workout.editing_id.is_some() {
                    fl!("workout-edit")
                } else {
                    fl!("workout-new")
                };
                context_drawer::context_drawer(
                    self.workout.settings_view().map(Message::Workout),
                    Message::ToggleContextPage(ContextPage::WorkoutEdit),
                )
                .title(title)
            }
            ContextPage::CountdownEdit => {
                let title = if self.countdown.editing_id.is_some() {
                    fl!("countdown-edit")
                } else {
                    fl!("countdown-new")
                };
                context_drawer::context_drawer(
                    self.countdown.settings_view(self.use_12h).map(Message::Countdown),
                    Message::ToggleContextPage(ContextPage::CountdownEdit),
                )
                .title(title)
            }
        })
    }

    fn view(&self) -> Element<'_, Self::Message> {
        if self.show_settings {
            let page = widget::container(self.settings_page())
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(16);
            return toaster::toaster(&self.toasts, page).into();
        }

        let content: Element<_> = match self.nav.active_data::<Page>() {
            Some(Page::WorldClocks) => self
                .world_clocks
                .view(self.use_12h, self.auto_sort_world_clocks)
                .map(Message::WorldClocks),
            Some(Page::Stopwatch) => self.stopwatch.view().map(Message::Stopwatch),
            Some(Page::Alarm) => self.alarm.view(self.use_12h, self.auto_sort_alarms).map(Message::Alarm),
            Some(Page::Timer) => self.timer.view().map(Message::Timer),
            Some(Page::Pomodoro) => self.pomodoro.view().map(Message::Pomodoro),
            Some(Page::Chess) => self.chess.view().map(Message::Chess),
            Some(Page::Workout) => self.workout.view().map(Message::Workout),
            Some(Page::Countdown) => self.countdown.view(self.use_12h).map(Message::Countdown),
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

        if self.show_palette {
            return Some(self.palette_view());
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

        // The daemon's half of the split. Same mechanism as the config watcher
        // above, so ringing and snooze changes arrive without a D-Bus client.
        subscriptions.push(
            self.core()
                .watch_state::<crate::runtime::RuntimeState>(Self::APP_ID)
                .map(|update| Message::UpdateRuntime(update.config)),
        );
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
                | Message::UpdateRuntime(_)
                // Fires per drag-motion event; the order is saved on finish.
                | Message::NavStartDrag(_)
                | Message::NavReorder(..)
                | Message::NavCancelDrag
                | Message::ShowSettings
                | Message::CloseShortcutsDialog
                | Message::ShowShortcutsDialog
                | Message::OpenPalette
                | Message::ClosePalette
                | Message::PaletteInput(_)
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
                    // Toast the alarm that was actually saved. Using the last list
                    // entry breaks when editing, and auto-sort may reorder the list
                    // during the save, making the position arbitrary.
                    if let Some(alarm) = self
                        .alarm
                        .last_saved_id
                        .and_then(|id| self.alarm.alarms.iter().find(|a| a.id == id))
                    {
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
                // The daemon owns ringing, so these are requests rather than
                // state changes. Fire and forget on a thread -- the resulting
                // state arrives back through the runtime watcher, which is also
                // what stops the audio. Doing it here as well would race.
                alarm::Message::SnoozeAlarm(alarm_id) => {
                    let alarm_id = *alarm_id;
                    daemon_call(move || crate::ipc::snooze(alarm_id));
                }
                alarm::Message::DismissAlarm(alarm_id) => {
                    let alarm_id = *alarm_id;
                    daemon_call(move || crate::ipc::dismiss(alarm_id));
                }
                _ => {
                    self.alarm.update(msg.clone(), self.use_12h);
                }
            },

            Message::Timer(ref msg) => match msg {
                // The daemon owns running timers, so these are requests rather
                // than state changes -- it is what keeps a timer counting down
                // with the window closed. The resulting state comes back
                // through the runtime watcher.
                timer::Message::StartTimer(id) => {
                    let id = *id;
                    // Keyboard shortcuts act on the last timer touched.
                    self.active_timer_id = Some(id);
                    daemon_call(move || crate::ipc::timer_start(id));
                }
                timer::Message::PauseTimer(id) => {
                    let id = *id;
                    self.active_timer_id = Some(id);
                    daemon_call(move || crate::ipc::timer_pause(id));
                }
                timer::Message::ResumeTimer(id) => {
                    let id = *id;
                    self.active_timer_id = Some(id);
                    daemon_call(move || crate::ipc::timer_resume(id));
                }
                timer::Message::ResetTimer(id) => {
                    let id = *id;
                    daemon_call(move || crate::ipc::timer_reset(id));
                }
                timer::Message::DeleteTimer(id) => {
                    let id = *id;
                    if self.confirm_delete_timer && self.pending_destructive_action.is_none() {
                        self.pending_destructive_action = Some(DestructiveAction::DeleteTimer(id));
                        self.confirm_dialog_dont_show_again = false;
                        return Task::none();
                    }
                    // Only once the delete is actually going ahead: stop it
                    // counting down before the definition it refers to is gone.
                    daemon_call(move || crate::ipc::timer_reset(id));
                    self.timer.update(msg.clone());
                    if self.context_page == ContextPage::TimerAdd {
                        self.timer.editing = false;
                        self.core.window.show_context = false;
                    }
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
                        // Track the saved timer as active. When editing, that is
                        // `edit_id`; only a freshly created timer is the last one.
                        let edited = self.timer.edit_id;
                        self.timer.update(msg.clone());
                        self.active_timer_id =
                            edited.or_else(|| self.timer.timers.last().map(|t| t.id));
                    } else {
                        self.timer.update(msg.clone());
                    }
                    self.core.window.show_context = false;
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
                    self.save_state();
                }
                stopwatch::Message::ResumeFromHistory(_) => {
                    self.stopwatch.update(msg.clone());
                    self.core.window.show_context = false;
                    self.save_state();
                }
                stopwatch::Message::Reset => {
                    self.stopwatch.update(msg.clone());
                    if self.auto_clear_stopwatch_history {
                        self.stopwatch.update(stopwatch::Message::ClearHistory);
                    }
                    self.save_state();
                }
                stopwatch::Message::Tick => {
                    self.stopwatch.update(msg.clone());
                }
                stopwatch::Message::ExportAllHistory => {
                    if !self.stopwatch.history.is_empty() {
                        let csv = self.stopwatch.history_csv();
                        return save_csv_dialog(fl!("export-filename-all"), csv);
                    }
                }
                stopwatch::Message::ExportRecord(id) => {
                    if let Some(csv) = self.stopwatch.record_csv(*id) {
                        return save_csv_dialog(fl!("export-filename-record"), csv);
                    }
                }
                _ => {
                    self.stopwatch.update(msg.clone());
                    self.save_state();
                }
            },

            Message::Pomodoro(ref msg) => match msg {
                pomodoro::Message::Delete(id) => {
                    let id = *id;
                    if self.confirm_delete_pomodoro && self.pending_destructive_action.is_none() {
                        self.pending_destructive_action =
                            Some(DestructiveAction::DeletePomodoro(id));
                        self.confirm_dialog_dont_show_again = false;
                        return Task::none();
                    }
                    // Stop it running before the definition it refers to is gone.
                    daemon_call(move || crate::ipc::pomodoro_reset(id));
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
                pomodoro::Message::CancelEditPomodoro
                | pomodoro::Message::SaveEditPomodoro
                | pomodoro::Message::AddTimer => {
                    // The saved timer is the one being edited, not the last in the
                    // list. `AddTimer` appends, so there `last()` is correct.
                    if matches!(msg, pomodoro::Message::SaveEditPomodoro) {
                        self.active_pomodoro_id = self.pomodoro.editing_id;
                    }
                    self.pomodoro.update(msg.clone());
                    if matches!(msg, pomodoro::Message::AddTimer)
                        && let Some(p) = self.pomodoro.timers.last()
                    {
                        self.active_pomodoro_id = Some(p.id);
                    }
                    // Close the drawer on every terminal action. `AddTimer`
                    // previously fell through to the catch-all and left it open.
                    self.core.window.show_context = false;
                }
                // The daemon owns the running session, so these are requests.
                // It advances work -> break -> work on its own, which is what
                // keeps a pomodoro cycling with the window closed.
                pomodoro::Message::Start(id) => {
                    let id = *id;
                    self.active_pomodoro_id = Some(id);
                    daemon_call(move || crate::ipc::pomodoro_start(id));
                }
                pomodoro::Message::Pause(id) => {
                    let id = *id;
                    self.active_pomodoro_id = Some(id);
                    daemon_call(move || crate::ipc::pomodoro_pause(id));
                }
                pomodoro::Message::Resume(id) => {
                    let id = *id;
                    self.active_pomodoro_id = Some(id);
                    daemon_call(move || crate::ipc::pomodoro_resume(id));
                }
                pomodoro::Message::Skip(id) => {
                    let id = *id;
                    self.active_pomodoro_id = Some(id);
                    daemon_call(move || crate::ipc::pomodoro_skip(id));
                }
                pomodoro::Message::Reset(id) => {
                    let id = *id;
                    daemon_call(move || crate::ipc::pomodoro_reset(id));
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

            Message::Chess(ref msg) => match msg {
                chess::Message::OpenSettings => {
                    self.chess.update(msg.clone());
                    self.context_page = ContextPage::ChessSettings;
                    self.core.window.show_context = true;
                    self.save_state();
                    return Task::none();
                }
                chess::Message::Tick => {
                    // Handled in handle_tick
                }
                _ => {
                    self.chess.update(msg.clone());
                }
            },

            Message::Workout(ref msg) => match msg {
                workout::Message::OpenSettings | workout::Message::StartEditWorkout(_) => {
                    self.workout.update(msg.clone());
                    self.context_page = ContextPage::WorkoutEdit;
                    self.core.window.show_context = true;
                    self.save_state();
                    return widget::text_input::focus(widget::Id::new("workout-label-input"));
                }
                workout::Message::CancelEditWorkout | workout::Message::SaveEditWorkout => {
                    self.workout.update(msg.clone());
                    self.core.window.show_context = false;
                }
                workout::Message::OpenBlockEditor(_) => {
                    // The block editor owns the whole page, so close the drawer
                    // behind it rather than leaving both open.
                    self.workout.update(msg.clone());
                    self.core.window.show_context = false;
                    self.save_state();
                }
                workout::Message::BrowseCustomSound => {
                    return open_sound_file_dialog(CustomSoundTarget::Workout);
                }
                workout::Message::Tick => {
                    // Handled in handle_tick
                }
                _ => {
                    self.workout.update(msg.clone());
                }
            },

            Message::Countdown(ref msg) => match msg {
                countdown::Message::OpenSettings | countdown::Message::StartEditEvent(_) => {
                    self.countdown.update(msg.clone());
                    self.context_page = ContextPage::CountdownEdit;
                    self.core.window.show_context = true;
                    self.save_state();
                    return widget::text_input::focus(widget::Id::new("countdown-label-input"));
                }
                countdown::Message::CancelEdit
                | countdown::Message::SaveEditEvent
                | countdown::Message::AddEvent => {
                    self.countdown.update(msg.clone());
                    self.core.window.show_context = false;
                }
                countdown::Message::BrowseCustomSound => {
                    return open_sound_file_dialog(CustomSoundTarget::Countdown);
                }
                countdown::Message::Tick => {
                    // Handled in handle_tick
                }
                _ => {
                    self.countdown.update(msg.clone());
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
                self.time_format = config.time_format;
                self.use_12h = self.time_format.use_12h();
                self.confirm_delete_alarm = config.confirm_delete_alarm;
                self.confirm_delete_timer = config.confirm_delete_timer;
                self.confirm_delete_world_clock = config.confirm_delete_world_clock;
                self.confirm_delete_pomodoro = config.confirm_delete_pomodoro;
                self.confirm_clear_stopwatch = config.confirm_clear_stopwatch;
                self.auto_sort_alarms = config.auto_sort_alarms;
                self.auto_sort_world_clocks = config.auto_sort_world_clocks;
                self.auto_clear_stopwatch_history = config.auto_clear_stopwatch_history;
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
                CustomSoundTarget::Workout => {
                    self.workout.update(workout::Message::EditSound(path));
                }
                CustomSoundTarget::Countdown => {
                    self.countdown.update(countdown::Message::EditSound(path));
                }
            },

            Message::SetTimeFormat(format) => {
                self.time_format = format;
                self.use_12h = format.use_12h();
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

            Message::ToggleNavPage(page, visible) => {
                if visible {
                    self.nav_hidden.retain(|p| *p != page);
                } else if !self.nav_hidden.contains(&page) {
                    // Refuse to hide the last page: an empty sidebar leaves the
                    // app on "select a view" with no way back.
                    let visible_count = self
                        .nav_order
                        .iter()
                        .filter(|p| !self.nav_hidden.contains(p))
                        .count();
                    if visible_count > 1 {
                        self.nav_hidden.push(page);
                    }
                }
                self.rebuild_nav();
                return self.update_title();
            }
            Message::ShowSettings => {
                self.show_settings = true;
                // Nothing else should be competing for the window.
                self.core.window.show_context = false;
                self.show_palette = false;
            }

            Message::NavStartDrag(index) => {
                self.nav_pre_drag = self.nav_order.clone();
                self.nav_dragging = Some(index);
            }
            Message::NavReorder(from, to) => {
                if from < self.nav_order.len() && to <= self.nav_order.len() && from != to {
                    let page = self.nav_order.remove(from);
                    let insert_at = if to > from { to - 1 } else { to };
                    let insert_at = insert_at.min(self.nav_order.len());
                    self.nav_order.insert(insert_at, page);
                    self.nav_dragging = Some(insert_at);
                    self.rebuild_nav();
                }
            }
            Message::NavFinishDrag => {
                self.nav_dragging = None;
                self.nav_pre_drag.clear();
            }
            Message::NavCancelDrag => {
                // Restore by id order: the list has already been mutated in
                // place by the Reorder messages seen during the drag.
                if !self.nav_pre_drag.is_empty() {
                    self.nav_order = std::mem::take(&mut self.nav_pre_drag);
                    self.rebuild_nav();
                }
                self.nav_dragging = None;
            }

            Message::UpdateRuntime(ref runtime) => {
                // Mirror the daemon's state into the fields the alarm views
                // already read, so nothing downstream has to know the schedule
                // moved out of process.
                self.alarm.ringing = runtime
                    .ringing
                    .iter()
                    .map(|r| alarm::RingingAlarm {
                        alarm_id: r.alarm_id,
                        label: r.label.clone(),
                        sound: r.sound.clone(),
                        ring_secs: r.ring_secs,
                        snooze_minutes: r.snooze_minutes,
                        // Only the daemon expires a ring, so this is display-only.
                        started_at: std::time::Instant::now(),
                    })
                    .collect();
                self.alarm.snoozed = runtime
                    .snoozed
                    .iter()
                    .map(|s| alarm::SnoozedAlarm {
                        alarm_id: s.alarm_id,
                        label: s.label.clone(),
                        sound: s.sound.clone(),
                        ring_minutes: s.ring_minutes,
                        snooze_minutes: s.snooze_minutes,
                        retrigger_at: s.retrigger_at,
                    })
                    .collect();
                // A spent one-shot reads as off. The daemon clears the flag once
                // the user switches the alarm back on.
                for id in &runtime.consumed_once {
                    if let Some(a) = self.alarm.alarms.iter_mut().find(|a| a.id == *id) {
                        a.is_enabled = false;
                    }
                }

                // Project the daemon's timer runs onto the fields the timer
                // views already read, so nothing downstream knows the countdown
                // moved out of process.
                let now = chrono::Local::now();
                for entry in &mut self.timer.timers {
                    match runtime.timer(entry.id) {
                        Some(run) => {
                            entry.is_running = run.is_running();
                            entry.remaining =
                                std::time::Duration::from_secs(run.remaining_secs(now));
                            entry.completed_count = run.completed;
                        }
                        None => {
                            entry.is_running = false;
                            entry.remaining = entry.initial_duration;
                            entry.completed_count = 0;
                        }
                    }
                }
                for entry in &mut self.pomodoro.timers {
                    match runtime.pomodoro(entry.id) {
                        Some(run) => {
                            entry.is_running = run.is_running();
                            entry.remaining =
                                std::time::Duration::from_secs(run.remaining_secs(now));
                            entry.session_type = match run.session {
                                crate::runtime::SessionKind::Work => {
                                    pomodoro::SessionType::Work
                                }
                                crate::runtime::SessionKind::ShortBreak => {
                                    pomodoro::SessionType::ShortBreak
                                }
                                crate::runtime::SessionKind::LongBreak => {
                                    pomodoro::SessionType::LongBreak
                                }
                            };
                            entry.session_number = run.session_number;
                            entry.completed_work_sessions = run.completed_work_sessions;
                        }
                        // No run: either never started, or reset. Restore the
                        // opening state -- clearing only `is_running` would
                        // leave a reset pomodoro frozen mid-session.
                        //
                        // Field-wise rather than rebuilding the entry, so the
                        // label, durations and custom sound survive.
                        None => {
                            let work = std::time::Duration::from_secs(
                                u64::from(entry.work_minutes) * 60,
                            );
                            entry.session_number = 1;
                            entry.session_type = pomodoro::SessionType::Work;
                            entry.remaining = work;
                            entry.started_remaining = work;
                            entry.is_running = false;
                            entry.start_instant = None;
                            entry.completed_work_sessions = 0;
                            entry.total_focused_secs = 0;
                        }
                    }
                }

                for event in &mut self.countdown.events {
                    event.fired = event
                        .reminders
                        .iter()
                        .copied()
                        .filter(|r| runtime.was_delivered(event.id, r.key()))
                        .collect();
                    event.arrived = runtime
                        .was_delivered(event.id, crate::runtime::CountdownDelivery::ARRIVED);
                }

                // Daily stats are ours to write, so fold in whatever the daemon
                // banked while we were closed and tell it to clear the counter.
                let banked: Vec<(u32, u64)> = runtime
                    .pomodoro
                    .iter()
                    .filter(|p| p.unrecorded_focus_secs > 0)
                    .map(|p| (p.timer_id, p.unrecorded_focus_secs))
                    .collect();
                for (timer_id, secs) in banked {
                    self.pomodoro.record_completed_work(secs);
                    daemon_call(move || crate::ipc::pomodoro_focus_recorded(timer_id, secs));
                }

                self.runtime = runtime.clone();
            }

            Message::OpenPalette => {
                self.show_palette = true;
                self.palette_input.clear();
                // Close anything that would fight the palette for focus.
                self.core.window.show_context = false;
                self.show_shortcuts_dialog = false;
                return widget::text_input::focus(widget::Id::new("palette-input"));
            }
            Message::ClosePalette => {
                self.show_palette = false;
                self.palette_input.clear();
            }
            Message::PaletteInput(text) => {
                self.palette_input = text;
            }
            Message::PaletteSubmit => {
                let action = crate::quick_action::parse(&self.palette_input);
                self.show_palette = false;
                self.palette_input.clear();
                if let Some(action) = action {
                    return self.run_quick_action(action);
                }
            }

            Message::PaletteRun(action) => {
                self.show_palette = false;
                self.palette_input.clear();
                return self.run_quick_action(action);
            }

            Message::CloseShortcutsDialog => {
                // Escape is a general "back out of the current thing". The
                // shortcuts dialog takes priority; otherwise it leaves the focus
                // mode of whichever page is showing one.
                if self.show_shortcuts_dialog {
                    self.show_shortcuts_dialog = false;
                } else if self.show_settings {
                    self.show_settings = false;
                } else {
                    match self.nav.active_data::<Page>() {
                        Some(Page::Timer) => self.timer.focused_id = None,
                        Some(Page::Workout) => self.workout.focused_id = None,
                        Some(Page::Pomodoro) => self.pomodoro.focused_id = None,
                        Some(Page::WorldClocks) => self.world_clocks.selected_clock_id = None,
                        _ => {}
                    }
                }
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
                        daemon_call(move || crate::ipc::timer_reset(id));
                        self.timer.update(timer::Message::DeleteTimer(id));
                        if self.context_page == ContextPage::TimerAdd {
                            self.timer.editing = false;
                            self.core.window.show_context = false;
                        }
                    }
                    Some(DestructiveAction::DeleteWorldClock(id)) => {
                        self.world_clocks.update(world_clocks::Message::RemoveClock(id));
                    }
                    Some(DestructiveAction::DeletePomodoro(id)) => {
                        daemon_call(move || crate::ipc::pomodoro_reset(id));
                        self.pomodoro.update(pomodoro::Message::Delete(id));
                    }
                    Some(DestructiveAction::ClearStopwatchHistory) => {
                        self.stopwatch.update(stopwatch::Message::ClearHistory);
                    }
                    None => {}
                }
                self.confirm_dialog_dont_show_again = false;
                self.save_state();
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

            Message::ExportFinished(text) => {
                return self
                    .toasts
                    .push(toaster::Toast::new(text))
                    .map(cosmic::action::app);
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

            Message::SetAutoClearStopwatchHistory(enabled) => {
                self.auto_clear_stopwatch_history = enabled;
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

    /// Another `clocks` invocation handed us its arguments instead of starting a
    /// second window. That is how a notification click reaches an app that was
    /// already open.
    fn dbus_activation(
        &mut self,
        msg: cosmic::dbus_activation::Message,
    ) -> Task<cosmic::Action<Self::Message>> {
        if let cosmic::dbus_activation::Details::ActivateAction { args, .. } = msg.msg
            && let Some(page) = args.first().and_then(|key| Page::from_key(key))
        {
            self.activate_page(page);
            self.core.window.show_context = false;
            return self.update_title();
        }
        Task::none()
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        self.nav.activate(id);
        self.core.window.show_context = false;
        // Settings renders ahead of the nav page, so without this a sidebar
        // click would move the highlight and change nothing on screen. The
        // sidebar is how you leave settings.
        self.show_settings = false;
        self.update_title()
    }
}

/// Keyboard shortcuts shown beside menu items.
///
/// `menu::items` looks each action up here and renders the binding; an empty map
/// means the menu shows no shortcuts at all. These must be kept in step with the
/// real bindings in `subscriptions::input_subscription`.
fn key_binds() -> HashMap<menu::KeyBind, MenuAction> {
    let mut binds = HashMap::new();
    binds.insert(
        menu::KeyBind {
            modifiers: vec![menu::key_bind::Modifier::Ctrl],
            key: cosmic::iced::keyboard::Key::Character("k".into()),
        },
        MenuAction::QuickAction,
    );
    binds
}
