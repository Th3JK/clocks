// SPDX-License-Identifier: MIT

use crate::audio;
use crate::config::{
    Config, PomodoroDefaults, SavedAlarm, SavedClock, SavedPomodoro, SavedRepeatMode, SavedTimer,
};
use crate::fl;
use crate::pages::{alarm, pomodoro, stopwatch, timer, world_clocks, ContextPage, Page};
use chrono::{Datelike, Local, Timelike};
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::Length;
use cosmic::iced::Subscription;
use cosmic::widget::{self, about::About, icon, menu, nav_bar};
use cosmic::{iced_futures, prelude::*};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");

// --- Model ---

pub struct AppModel {
    core: cosmic::Core,
    context_page: ContextPage,
    about: About,
    nav: nav_bar::Model,
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    config: Config,
    config_context: Option<cosmic_config::Config>,

    // Page states (each page owns its own MVU model)
    world_clocks: world_clocks::WorldClocksState,
    stopwatch: stopwatch::StopwatchState,
    alarm: alarm::AlarmState,
    timer: timer::TimerState,
    pomodoro: pomodoro::PomodoroState,

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
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
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

        let mut app = AppModel {
            core,
            context_page: ContextPage::default(),
            about,
            nav,
            key_binds: HashMap::new(),
            config,
            config_context,
            world_clocks,
            stopwatch: stopwatch::StopwatchState::default(),
            alarm,
            timer,
            pomodoro,
            alarm_audio_stops: HashMap::new(),
        };

        let command = app.update_title();
        (app, command)
    }

    // --- View ---

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("view")).apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![menu::Item::Button(fl!("about"), None, MenuAction::About)],
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
                    self.alarm.sidebar_view().map(Message::Alarm),
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
        })
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let content: Element<_> = match self.nav.active_data::<Page>() {
            Some(Page::WorldClocks) => self.world_clocks.view().map(Message::WorldClocks),
            Some(Page::Stopwatch) => self.stopwatch.view().map(Message::Stopwatch),
            Some(Page::Alarm) => self.alarm.view().map(Message::Alarm),
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
        let ringing = self.alarm.ringing.first()?;
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
        Some(dialog.into())
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subscriptions = vec![
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(update.config)),
        ];

        subscriptions.push(Subscription::run(tick_subscription));

        Subscription::batch(subscriptions)
    }

    // --- Update ---

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        let should_save = !matches!(message, Message::Tick | Message::UpdateConfig(_));

        match message {
            Message::Tick => {
                self.handle_tick();
            }

            Message::WorldClocks(ref msg) => match msg {
                world_clocks::Message::OpenAddSidebar => {
                    self.context_page = ContextPage::WorldClocksAdd;
                    self.core.window.show_context = true;
                }
                _ => {
                    self.world_clocks.update(msg.clone());
                }
            },

            Message::Alarm(ref msg) => match msg {
                alarm::Message::StartNewAlarm | alarm::Message::StartEditAlarm(_) => {
                    self.alarm.update(msg.clone());
                    self.context_page = ContextPage::AlarmEdit;
                    self.core.window.show_context = true;
                }
                alarm::Message::CancelEdit | alarm::Message::SaveAlarm => {
                    self.alarm.update(msg.clone());
                    self.core.window.show_context = false;
                }
                alarm::Message::BrowseCustomSound => {
                    return open_sound_file_dialog(CustomSoundTarget::Alarm);
                }
                alarm::Message::SnoozeAlarm(alarm_id) => {
                    let alarm_id = *alarm_id;
                    self.stop_alarm_audio(alarm_id);
                    self.alarm.update(msg.clone());
                }
                alarm::Message::DismissAlarm(alarm_id) => {
                    let alarm_id = *alarm_id;
                    self.stop_alarm_audio(alarm_id);
                    self.alarm.update(msg.clone());
                }
                _ => {
                    self.alarm.update(msg.clone());
                }
            },

            Message::Timer(ref msg) => match msg {
                timer::Message::StartNew | timer::Message::StartEditTimer(_) => {
                    self.timer.update(msg.clone());
                    self.context_page = ContextPage::TimerAdd;
                    self.core.window.show_context = true;
                }
                timer::Message::CancelEdit | timer::Message::SaveTimer => {
                    self.timer.update(msg.clone());
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
                _ => {
                    self.stopwatch.update(msg.clone());
                }
            },

            Message::Pomodoro(ref msg) => match msg {
                pomodoro::Message::OpenSettings | pomodoro::Message::StartEditPomodoro(_) => {
                    self.pomodoro.update(msg.clone());
                    self.context_page = ContextPage::PomodoroSettings;
                    self.core.window.show_context = true;
                }
                pomodoro::Message::CancelEditPomodoro | pomodoro::Message::SaveEditPomodoro => {
                    self.pomodoro.update(msg.clone());
                    self.core.window.show_context = false;
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
                self.config = config;
            }

            Message::CustomSoundSelected(target, path) => {
                match target {
                    CustomSoundTarget::Alarm => {
                        self.alarm.update(alarm::Message::EditSound(path));
                    }
                    CustomSoundTarget::Timer => {
                        self.timer.update(timer::Message::EditSound(path));
                    }
                    CustomSoundTarget::Pomodoro => {
                        self.pomodoro.update(pomodoro::Message::EditSound(path));
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
            self.alarm.update(alarm::Message::SnoozeAlarm(alarm_id));
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

fn open_sound_file_dialog(target: CustomSoundTarget) -> Task<cosmic::Action<Message>> {
    cosmic::task::future(async move {
        let dialog = cosmic::dialog::file_chooser::open::Dialog::new()
            .title(crate::fl!("choose-sound-file"));

        match dialog.open_file().await {
            Ok(response) => {
                let path = response.url().path().to_string();
                cosmic::Action::App(Message::CustomSoundSelected(target, path))
            }
            Err(_) => {
                cosmic::Action::App(Message::Tick)
            }
        }
    })
}

// --- Persistence: build Config from runtime state ---

fn build_config_from_state(
    wc: &world_clocks::WorldClocksState,
    al: &alarm::AlarmState,
    ti: &timer::TimerState,
    po: &pomodoro::PomodoroState,
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
                alarm::RepeatMode::Custom(days) => {
                    SavedRepeatMode::Custom(days.iter().map(|d| d.short_name().to_string()).collect())
                }
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
    let mut state = pomodoro::PomodoroState::default();

    // Apply saved defaults
    state.default_work_minutes = config.pomodoro_defaults.work_minutes;
    state.default_short_break_minutes = config.pomodoro_defaults.short_break_minutes;
    state.default_long_break_minutes = config.pomodoro_defaults.long_break_minutes;

    if !config.pomodoros.is_empty() {
        state.timers.clear();
        for (i, p) in config.pomodoros.iter().enumerate() {
            let mut timer =
                pomodoro::PomodoroTimer::from_config(i as u32, p.label.clone(), p.work_minutes, p.short_break_minutes, p.long_break_minutes);
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
