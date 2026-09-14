// SPDX-License-Identifier: MIT

//! The background daemon. Owns alarm scheduling, audio and notifications.
//!
//! The GUI does not schedule anything: it renders, edits definitions, and sends
//! Snooze/Dismiss here. That is the point of the split -- an alarm fires whether
//! or not a window is open, and there is exactly one owner of time, so there is
//! no way for both processes to ring at once.
//!
//! Started two ways, neither of which depends on systemd:
//!
//! - D-Bus activation, so the GUI starting is enough to bring it up.
//! - An autostart entry, so it is running before the GUI ever opens.

use clocks::config::Config;
use clocks::runtime::{
    CountdownDelivery, PomodoroRun, RingingRecord, RuntimeState, SessionKind, TimerRun,
};
use clocks::{audio, ipc, scheduler};
use cosmic_config::CosmicConfigEntry;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How often the schedule is evaluated.
///
/// One second, not the GUI's 100 ms: nothing here needs sub-second resolution,
/// and `due_alarms` works on a window rather than an instant, so a slow or
/// delayed tick cannot miss an alarm the way the old minute-bucket could.
const TICK: Duration = Duration::from_secs(1);

/// Shared between the D-Bus interface and the scheduler thread.
struct Daemon {
    state: Mutex<RuntimeState>,
    state_ctx: Option<cosmic_config::Config>,
    config_ctx: Option<cosmic_config::Config>,
    /// Stop flags for the looping alarm audio, keyed by alarm id.
    audio_stops: Mutex<HashMap<u32, Arc<AtomicBool>>>,
    /// The checkpoint that last reached disk, for deciding when to write again.
    /// In-memory state advances every tick, so it cannot answer that question.
    last_persisted: Mutex<Option<chrono::DateTime<chrono::Local>>>,
}

impl Daemon {
    fn persist(&self, state: &RuntimeState) {
        if let Some(ctx) = &self.state_ctx
            && let Err(e) = state.write_entry(ctx)
        {
            eprintln!("clocks-daemon: failed to write runtime state: {e:?}");
            return;
        }
        *self.last_persisted.lock().expect("checkpoint poisoned") = state.checked_through;
    }

    fn definitions(&self) -> Config {
        self.config_ctx
            .as_ref()
            .map(|ctx| match Config::get_entry(ctx) {
                Ok(config) => config,
                Err((_errors, config)) => config,
            })
            .unwrap_or_default()
    }

    fn start_audio(&self, alarm_id: u32, sound: &str, ring_secs: u64) {
        let stop = Arc::new(AtomicBool::new(false));
        self.audio_stops
            .lock()
            .expect("audio stop registry poisoned")
            .insert(alarm_id, stop.clone());

        let sound = sound.to_string();
        std::thread::spawn(move || {
            if let Err(e) = audio::play_alarm_sound_loop(&sound, ring_secs, stop) {
                eprintln!("clocks-daemon: alarm audio failed: {e}");
            }
        });
    }

    fn stop_audio(&self, alarm_id: u32) {
        if let Some(stop) = self
            .audio_stops
            .lock()
            .expect("audio stop registry poisoned")
            .remove(&alarm_id)
        {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Stop an alarm ringing. Shared by the D-Bus method and the notification's
    /// own Dismiss button.
    fn answer_dismiss(&self, alarm_id: u32) {
        self.stop_audio(alarm_id);
        let mut state = self.state.lock().expect("runtime state poisoned");
        scheduler::dismiss(&mut state, alarm_id);
        self.persist(&state);
    }

    /// Stop an alarm ringing and re-ring after its snooze interval.
    fn answer_snooze(&self, alarm_id: u32) {
        self.stop_audio(alarm_id);
        let mut state = self.state.lock().expect("runtime state poisoned");
        scheduler::snooze(&mut state, alarm_id, chrono::Local::now());
        self.persist(&state);
    }

    /// Look a timer definition up by id.
    fn timer_def(&self, timer_id: u32) -> Option<clocks::config::SavedTimer> {
        self.definitions()
            .timers
            .into_iter()
            .find(|t| t.id == timer_id)
    }

    /// Apply a change to one timer's run state and persist.
    ///
    /// Every control goes through here so there is one place that writes, and
    /// so the GUI's watcher sees exactly one update per command.
    fn update_timers(&self, f: impl FnOnce(&mut Vec<TimerRun>)) {
        let mut state = self.state.lock().expect("runtime state poisoned");
        f(&mut state.timers);
        self.persist(&state);
    }

    fn timer_start(&self, timer_id: u32) {
        let Some(def) = self.timer_def(timer_id) else {
            return;
        };
        let now = chrono::Local::now();
        self.update_timers(|runs| {
            runs.retain(|t| t.timer_id != timer_id);
            runs.push(TimerRun {
                timer_id,
                deadline: Some(now + chrono::Duration::seconds(def.duration_secs as i64)),
                remaining_secs: def.duration_secs,
                completed: 0,
            });
        });
    }

    fn timer_pause(&self, timer_id: u32) {
        let now = chrono::Local::now();
        self.update_timers(|runs| {
            if let Some(run) = runs.iter_mut().find(|t| t.timer_id == timer_id) {
                run.remaining_secs = run.remaining_secs(now);
                run.deadline = None;
            }
        });
    }

    fn timer_resume(&self, timer_id: u32) {
        let now = chrono::Local::now();
        self.update_timers(|runs| {
            if let Some(run) = runs.iter_mut().find(|t| t.timer_id == timer_id)
                && run.deadline.is_none()
            {
                run.deadline = Some(now + chrono::Duration::seconds(run.remaining_secs as i64));
            }
        });
    }

    fn timer_reset(&self, timer_id: u32) {
        self.update_timers(|runs| runs.retain(|t| t.timer_id != timer_id));
    }

    fn pomodoro_def(&self, timer_id: u32) -> Option<clocks::config::SavedPomodoro> {
        self.definitions()
            .pomodoros
            .into_iter()
            .find(|p| p.id == timer_id)
    }

    fn update_pomodoro(&self, f: impl FnOnce(&mut Vec<PomodoroRun>)) {
        let mut state = self.state.lock().expect("runtime state poisoned");
        f(&mut state.pomodoro);
        self.persist(&state);
    }

    fn pomodoro_start(&self, timer_id: u32) {
        let Some(def) = self.pomodoro_def(timer_id) else {
            return;
        };
        let secs = u64::from(def.work_minutes) * 60;
        let now = chrono::Local::now();
        self.update_pomodoro(|runs| {
            runs.retain(|p| p.timer_id != timer_id);
            runs.push(PomodoroRun {
                timer_id,
                session: SessionKind::Work,
                session_number: 1,
                deadline: Some(now + chrono::Duration::seconds(secs as i64)),
                remaining_secs: secs,
                completed_work_sessions: 0,
                unrecorded_focus_secs: 0,
            });
        });
    }

    fn pomodoro_pause(&self, timer_id: u32) {
        let now = chrono::Local::now();
        self.update_pomodoro(|runs| {
            if let Some(run) = runs.iter_mut().find(|p| p.timer_id == timer_id) {
                run.remaining_secs = run.remaining_secs(now);
                run.deadline = None;
            }
        });
    }

    fn pomodoro_resume(&self, timer_id: u32) {
        let now = chrono::Local::now();
        self.update_pomodoro(|runs| {
            if let Some(run) = runs.iter_mut().find(|p| p.timer_id == timer_id)
                && run.deadline.is_none()
            {
                run.deadline = Some(now + chrono::Duration::seconds(run.remaining_secs as i64));
            }
        });
    }

    /// Jump straight to the next phase without waiting it out.
    fn pomodoro_skip(&self, timer_id: u32) {
        let Some(def) = self.pomodoro_def(timer_id) else {
            return;
        };
        let now = chrono::Local::now();
        self.update_pomodoro(|runs| {
            if let Some(run) = runs.iter_mut().find(|p| p.timer_id == timer_id) {
                advance_session(run, &def, now);
            }
        });
    }

    fn pomodoro_reset(&self, timer_id: u32) {
        self.update_pomodoro(|runs| runs.retain(|p| p.timer_id != timer_id));
    }

    /// Ring an alarm: notification with actions, looping audio, recorded state.
    /// Takes `&Arc<Self>` so the notification thread can hold the daemon and
    /// answer Dismiss/Snooze without a bus round-trip.
    fn start_ringing(self: &Arc<Self>, state: &mut RuntimeState, due: scheduler::DueAlarm) {
        if state.is_ringing(due.alarm_id) {
            return;
        }

        notify(Arc::clone(self), due.alarm_id, &due.label);
        self.start_audio(due.alarm_id, &due.sound, due.ring_secs);

        state.ringing.push(RingingRecord {
            alarm_id: due.alarm_id,
            label: due.label,
            sound: due.sound,
            ring_secs: due.ring_secs,
            snooze_minutes: due.snooze_minutes,
            started_at: chrono::Local::now(),
        });
    }
}

/// Post the ringing notification and act on whichever button is pressed.
///
/// With no window open the notification is the *only* way to answer an alarm,
/// so Dismiss and Snooze have to live here. `wait_for_action` blocks until the
/// user acts or the notification closes, hence the thread.
fn notify(daemon: Arc<Daemon>, alarm_id: u32, label: &str) {
    let body = label.to_string();
    std::thread::spawn(move || {
        let handle = notify_rust::Notification::new()
            .summary(&clocks::fl!("notification-alarm"))
            .body(&body)
            .icon("alarm-symbolic")
            // The "default" key is what makes the notification *body*
            // activatable. Without it the server emits no ActionInvoked for a
            // body click -- it just closes the notification, which reads as the
            // click having dismissed the alarm. Servers conventionally render
            // "default" as the body rather than a third button.
            .action("default", &clocks::fl!("notification-open"))
            .action("dismiss", &clocks::fl!("dismiss"))
            .action("snooze", &clocks::fl!("snooze"))
            // Must not time out: a ringing alarm stays until answered.
            .timeout(notify_rust::Timeout::Never)
            .show();

        match handle {
            Ok(handle) => handle.wait_for_action(|action| match action {
                // Called directly, *not* back through D-Bus. This closure runs
                // inside notify-rust's own zbus runtime, and a blocking zbus
                // call from there panics with "Cannot start a runtime from
                // within a runtime". The daemon has no business calling itself
                // over the bus anyway.
                "dismiss" => daemon.answer_dismiss(alarm_id),
                "snooze" => daemon.answer_snooze(alarm_id),
                // Clicking the body opens the app on the Alarm page. The alarm
                // keeps ringing -- opening is not the same as answering, and the
                // window has its own Dismiss and Snooze.
                "default" => open_app_at(clocks::pages::Page::Alarm),
                // Closing the notification is not an answer either; the alarm
                // rings on until its window expires and it auto-snoozes.
                _ => {}
            }),
            Err(e) => eprintln!("clocks-daemon: notification failed: {e}"),
        }
    });
}

/// Launch the GUI on a given page.
///
/// `run_single_instance` in the GUI does the hard part: if a window is already
/// open this is forwarded to it and the new process exits, so this both opens
/// and focuses without the daemon needing to know which case it is in.
fn open_app_at(page: clocks::pages::Page) {
    // Alongside the daemon first, so a dev build launches its sibling rather
    // than an installed copy of a different vintage.
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("clocks")))
        .filter(|p| p.exists());

    let program = sibling.unwrap_or_else(|| std::path::PathBuf::from("clocks"));
    if let Err(e) = std::process::Command::new(&program).arg(page.key()).spawn() {
        eprintln!("clocks-daemon: could not open the app: {e}");
    }
}

/// Move a pomodoro to its next phase and re-arm the deadline.
///
/// Mirrors `PomodoroTimer::advance_session`, which stays in the GUI for the
/// case where it is driving the display. Work goes to a long break every fourth
/// completed work session and a short break otherwise; a break goes back to
/// work and bumps the session number.
///
/// Returns the label for the phase just entered, for the notification body.
fn advance_session(
    run: &mut PomodoroRun,
    def: &clocks::config::SavedPomodoro,
    now: chrono::DateTime<chrono::Local>,
) -> SessionKind {
    let minutes = match run.session {
        SessionKind::Work => {
            run.completed_work_sessions += 1;
            // Banked for the GUI to fold into daily stats, which the daemon
            // cannot write.
            run.unrecorded_focus_secs += u64::from(def.work_minutes) * 60;
            if run.completed_work_sessions % 4 == 0 {
                run.session = SessionKind::LongBreak;
                def.long_break_minutes
            } else {
                run.session = SessionKind::ShortBreak;
                def.short_break_minutes
            }
        }
        SessionKind::ShortBreak | SessionKind::LongBreak => {
            run.session_number += 1;
            run.session = SessionKind::Work;
            def.work_minutes
        }
    };

    let secs = u64::from(minutes) * 60;
    run.remaining_secs = secs;
    run.deadline = Some(now + chrono::Duration::seconds(secs as i64));
    run.session
}

/// Post a notification whose body opens the app on `page`.
///
/// For things that have simply happened and need no answer -- a finished timer,
/// a countdown reminder. A ringing alarm uses `notify` instead, which adds
/// Dismiss and Snooze; the shared part is only the body behaviour.
///
/// The `default` action is what makes the body clickable at all: without it the
/// server emits no ActionInvoked and merely closes the banner.
fn notify_opening(page: clocks::pages::Page, summary: String, body: String) {
    std::thread::spawn(move || {
        let handle = notify_rust::Notification::new()
            .summary(&summary)
            .body(&body)
            .icon("alarm-symbolic")
            .action("default", &clocks::fl!("notification-open"))
            .show();

        match handle {
            // Blocks until the user acts or the banner closes, which is why
            // this needs its own thread.
            Ok(handle) => handle.wait_for_action(|action| {
                if action == "default" {
                    open_app_at(page);
                }
            }),
            Err(e) => eprintln!("clocks-daemon: notification failed: {e}"),
        }
    });
}

struct DaemonInterface(Arc<Daemon>);

#[zbus::interface(name = "dev.sramek.clocks.Daemon")]
impl DaemonInterface {
    fn dismiss(&self, alarm_id: u32) {
        self.0.answer_dismiss(alarm_id);
    }

    fn snooze(&self, alarm_id: u32) {
        self.0.answer_snooze(alarm_id);
    }

    fn timer_start(&self, timer_id: u32) {
        self.0.timer_start(timer_id);
    }

    fn timer_pause(&self, timer_id: u32) {
        self.0.timer_pause(timer_id);
    }

    fn timer_resume(&self, timer_id: u32) {
        self.0.timer_resume(timer_id);
    }

    fn timer_reset(&self, timer_id: u32) {
        self.0.timer_reset(timer_id);
    }

    fn pomodoro_start(&self, timer_id: u32) {
        self.0.pomodoro_start(timer_id);
    }

    fn pomodoro_pause(&self, timer_id: u32) {
        self.0.pomodoro_pause(timer_id);
    }

    fn pomodoro_resume(&self, timer_id: u32) {
        self.0.pomodoro_resume(timer_id);
    }

    fn pomodoro_skip(&self, timer_id: u32) {
        self.0.pomodoro_skip(timer_id);
    }

    fn pomodoro_reset(&self, timer_id: u32) {
        self.0.pomodoro_reset(timer_id);
    }

    /// The GUI has folded `secs` into daily stats; stop banking them.
    fn pomodoro_focus_recorded(&self, timer_id: u32, secs: u64) {
        self.0.update_pomodoro(|runs| {
            if let Some(run) = runs.iter_mut().find(|p| p.timer_id == timer_id) {
                run.unrecorded_focus_secs = run.unrecorded_focus_secs.saturating_sub(secs);
            }
        });
    }

    /// Deliberately empty, and deliberately kept.
    ///
    /// This exists purely as a D-Bus activation target: calling *any* method on
    /// the interface is what starts the daemon, and the GUI calls this one at
    /// launch for exactly that reason. The scheduler re-reads definitions every
    /// tick, so there is nothing for it to do beyond existing.
    fn reload(&self) {}
}

/// One pass of the schedule.
fn tick(daemon: &Arc<Daemon>) {
    let now = chrono::Local::now();
    let config = daemon.definitions();
    let alarms = clocks::app::persistence::restore_alarms(&config).alarms;

    let mut state = daemon.state.lock().expect("runtime state poisoned");
    let before = state.clone();

    // A ring left unanswered auto-snoozes rather than simply stopping, which is
    // what the app has always done.
    for expired in scheduler::expired_rings(&state, now) {
        daemon.stop_audio(expired.alarm_id);
        scheduler::snooze(&mut state, expired.alarm_id, now);
    }

    for snooze in scheduler::due_snoozes(&state, now) {
        state.snoozed.retain(|s| s.alarm_id != snooze.alarm_id);
        daemon.start_ringing(
            &mut state,
            scheduler::DueAlarm {
                alarm_id: snooze.alarm_id,
                label: snooze.label,
                sound: snooze.sound,
                ring_secs: u64::from(snooze.ring_minutes) * 60,
                snooze_minutes: snooze.snooze_minutes,
            },
        );
    }

    // Timers. Re-arming a repeat here is the point of the whole exercise: it is
    // what keeps a repeating timer going with no window open.
    let timer_defs = config.timers.clone();
    let mut expired: Vec<(TimerRun, clocks::config::SavedTimer)> = Vec::new();
    state.timers.retain_mut(|run| {
        let Some(deadline) = run.deadline else {
            return true;
        };
        if deadline > now {
            return true;
        }
        let Some(def) = timer_defs.iter().find(|d| d.id == run.timer_id) else {
            // Definition deleted while running -- drop the run rather than
            // firing something with no label or sound.
            return false;
        };

        run.completed += 1;
        expired.push((run.clone(), def.clone()));

        let repeats_left =
            def.repeat_enabled && (def.repeat_count == 0 || run.completed < def.repeat_count);
        if repeats_left {
            run.deadline = Some(now + chrono::Duration::seconds(def.duration_secs as i64));
            run.remaining_secs = def.duration_secs;
            true
        } else {
            false
        }
    });
    for (_run, def) in expired {
        notify_opening(
            clocks::pages::Page::Timer,
            clocks::fl!("notification-timer-complete"),
            def.label.clone(),
        );
        audio::play_sound(&def.sound);
    }

    // Pomodoro. Advancing the cycle here is what lets a session run unattended
    // -- the page's own advance_session only runs while the window is open.
    let pomodoro_defs = config.pomodoros.clone();
    let mut advanced: Vec<(String, String, SessionKind, SessionKind)> = Vec::new();
    state.pomodoro.retain_mut(|run| {
        let Some(deadline) = run.deadline else {
            return true;
        };
        if deadline > now {
            return true;
        }
        let Some(def) = pomodoro_defs.iter().find(|d| d.id == run.timer_id) else {
            return false;
        };
        let previous = run.session;
        let next = advance_session(run, def, now);
        advanced.push((def.label.clone(), def.sound.clone(), previous, next));
        true
    });
    for (label, sound, previous, next) in advanced {
        notify_opening(
            clocks::pages::Page::Pomodoro,
            clocks::fl!("notification-pomodoro"),
            clocks::fl!(
                "pomodoro-transition",
                label = label,
                prev = previous.display_name(),
                next = next.display_name()
            ),
        );
        audio::play_sound(&sound);
    }

    // Countdown: delivery only -- nothing to start or pause. Reminders fire in
    // threshold order, furthest-out first, so an event whose thresholds were all
    // crossed while nothing was running does not arrive as a jumbled burst.
    let mut deliveries: Vec<(String, String, clocks::pages::countdown::Reminder, u32)> =
        Vec::new();
    let mut arrivals: Vec<(String, String, u32)> = Vec::new();
    for event in &config.countdown_events {
        let target = event.target;
        let secs_until = target.signed_duration_since(now).num_seconds();
        let passed = secs_until <= 0;

        // Target back in the future while we still hold an arrival record means
        // the event was re-armed -- a yearly one rolled forward by the GUI, or
        // the date was edited. Forget its deliveries so next year fires. Without
        // this a yearly event would notify exactly once, ever.
        if !passed && state.was_delivered(event.id, CountdownDelivery::ARRIVED) {
            state
                .countdown_delivered
                .retain(|d| d.event_id != event.id);
        }

        if !passed {
            let mut due: Vec<clocks::pages::countdown::Reminder> = event
                .reminders
                .iter()
                .filter_map(|k| clocks::pages::countdown::Reminder::from_key(k))
                .filter(|r| {
                    secs_until <= r.secs_before() && !state.was_delivered(event.id, r.key())
                })
                .collect();
            due.sort_by_key(|r| std::cmp::Reverse(r.secs_before()));
            for reminder in due {
                state.countdown_delivered.push(CountdownDelivery {
                    event_id: event.id,
                    key: reminder.key().to_string(),
                });
                deliveries.push((event.label.clone(), event.sound.clone(), reminder, event.id));
            }
        } else if !state.was_delivered(event.id, CountdownDelivery::ARRIVED) {
            state.countdown_delivered.push(CountdownDelivery {
                event_id: event.id,
                key: CountdownDelivery::ARRIVED.to_string(),
            });
            arrivals.push((event.label.clone(), event.sound.clone(), event.id));
        }
    }

    // Delivery records for events that no longer exist are dead weight.
    let live: Vec<u32> = config.countdown_events.iter().map(|e| e.id).collect();
    state
        .countdown_delivered
        .retain(|d| live.contains(&d.event_id));

    for (label, sound, reminder, _id) in deliveries {
        notify_opening(
            clocks::pages::Page::Countdown,
            clocks::fl!("notification-countdown"),
            clocks::fl!(
                "countdown-reminder-body",
                label = label,
                when = reminder.display_name()
            ),
        );
        audio::play_sound(&sound);
    }
    for (label, sound, _id) in arrivals {
        notify_opening(
            clocks::pages::Page::Countdown,
            clocks::fl!("notification-countdown"),
            clocks::fl!("countdown-arrived", label = label),
        );
        audio::play_sound(&sound);
    }

    // Switching a spent one-shot back on in the GUI re-arms it. The GUI cannot
    // write here, so the daemon infers it from the definition being enabled.
    state
        .consumed_once
        .retain(|id| alarms.iter().any(|a| a.id == *id && !a.is_enabled));

    let since = state.checked_through.unwrap_or(now);
    for due in scheduler::due_alarms(&alarms, &state, since, now) {
        // A one-shot alarm is spent once it fires. Recorded here rather than by
        // clearing `is_enabled`, which is a definition field the GUI owns.
        if matches!(
            alarms.iter().find(|a| a.id == due.alarm_id).map(|a| &a.repeat_mode),
            Some(clocks::pages::alarm::RepeatMode::Once)
        ) {
            state.consumed_once.push(due.alarm_id);
        }
        daemon.start_ringing(&mut state, due);
    }
    state.checked_through = Some(now);

    // Two reasons to write, and they want different cadences.
    //
    // Something the GUI renders changed -- write immediately, it is watching.
    let substantive = state.ringing != before.ringing
        || state.snoozed != before.snoozed
        || state.consumed_once != before.consumed_once
        || state.timers != before.timers
        || state.pomodoro != before.pomodoro
        || state.countdown_delivered != before.countdown_delivered;

    // Or the checkpoint has drifted. This has to reach disk or the catch-up
    // window is wrong after a restart: `since` would fall back to `now` and
    // anything due while the daemon was down is skipped. Writing it every
    // second would wake the GUI's watcher every second, so it goes at a
    // coarser cadence -- well inside the 24h cap, and the worst case is
    // re-firing an alarm from the last half minute before a crash.
    let drifted = daemon
        .last_persisted
        .lock()
        .expect("checkpoint poisoned")
        .is_none_or(|last| now.signed_duration_since(last).num_seconds().abs() >= 30);

    if substantive || drifted {
        daemon.persist(&state);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    clocks::i18n::init(&requested_languages);

    let state_ctx = RuntimeState::context();
    let config_ctx = cosmic_config::Config::new(clocks::APP_ID, Config::VERSION).ok();

    let mut state = state_ctx
        .as_ref()
        .map(RuntimeState::load)
        .unwrap_or_default();

    let daemon = Arc::new(Daemon {
        state: Mutex::new(RuntimeState::default()),
        state_ctx,
        config_ctx,
        audio_stops: Mutex::new(HashMap::new()),
        last_persisted: Mutex::new(None),
    });

    // Nothing can still be ringing across a restart -- the audio thread died
    // with the old process. Anything that was is treated as unanswered and
    // snoozed, which is what would have happened had the ring simply expired.
    let now = chrono::Local::now();
    let interrupted = std::mem::take(&mut state.ringing);
    for ringing in interrupted {
        state.snoozed.retain(|s| s.alarm_id != ringing.alarm_id);
        state.snoozed.push(clocks::runtime::SnoozeRecord {
            alarm_id: ringing.alarm_id,
            label: ringing.label,
            sound: ringing.sound,
            ring_minutes: (ringing.ring_secs / 60) as u8,
            snooze_minutes: ringing.snooze_minutes,
            retrigger_at: now
                + chrono::Duration::minutes(i64::from(ringing.snooze_minutes)),
        });
    }
    *daemon.state.lock().expect("runtime state poisoned") = state;

    let _connection = zbus::blocking::connection::Builder::session()?
        .name(ipc::DAEMON_BUS_NAME)?
        .serve_at(ipc::DAEMON_PATH, DaemonInterface(daemon.clone()))?
        .build()?;

    loop {
        tick(&daemon);
        std::thread::sleep(TICK);
    }
}
