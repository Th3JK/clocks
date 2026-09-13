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

        // A config written before `time_format` existed carries `None`; fall back
        // to the legacy flag so an existing user keeps the display they had.
        let time_format = config.time_format.unwrap_or(if config.use_12h {
            crate::time_format::TimeFormat::Twelve
        } else {
            crate::time_format::TimeFormat::TwentyFour
        });
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
            alarm_audio_stops: HashMap::new(),
            toasts: toaster::Toasts::new(Message::CloseToast),
        };

        app.rebuild_nav();

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
                self.time_format = config.time_format.unwrap_or(if config.use_12h {
                    crate::time_format::TimeFormat::Twelve
                } else {
                    crate::time_format::TimeFormat::TwentyFour
                });
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
            Message::MoveNavPage(from, to) => {
                // Both indices are produced by the settings rows and so are
                // always in range; bounds-check anyway rather than risk a panic
                // on a view built from stale state.
                if from != to && from < self.nav_order.len() && to < self.nav_order.len() {
                    self.nav_order.swap(from, to);
                    self.rebuild_nav();
                }
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

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        self.nav.activate(id);
        self.core.window.show_context = false;
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
