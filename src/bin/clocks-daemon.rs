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
use clocks::runtime::{RingingRecord, RuntimeState};
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
}

impl Daemon {
    fn persist(&self, state: &RuntimeState) {
        if let Some(ctx) = &self.state_ctx
            && let Err(e) = state.write_entry(ctx)
        {
            eprintln!("clocks-daemon: failed to write runtime state: {e:?}");
        }
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
                // Closing the notification is not an answer -- the alarm keeps
                // ringing until its window expires and it auto-snoozes.
                _ => {}
            }),
            Err(e) => eprintln!("clocks-daemon: notification failed: {e}"),
        }
    });
}

struct DaemonInterface(Arc<Daemon>);

#[zbus::interface(name = "dev.th3jk.clocks.Daemon")]
impl DaemonInterface {
    fn dismiss(&self, alarm_id: u32) {
        self.0.answer_dismiss(alarm_id);
    }

    fn snooze(&self, alarm_id: u32) {
        self.0.answer_snooze(alarm_id);
    }

    /// Nudge to re-read definitions. The scheduler reads them every tick, so
    /// this exists mainly as an activation target for the GUI.
    fn reload(&self) {}

    /// Alarm ids currently ringing.
    fn list_ringing(&self) -> Vec<u32> {
        self.0
            .state
            .lock()
            .expect("runtime state poisoned")
            .ringing
            .iter()
            .map(|r| r.alarm_id)
            .collect()
    }
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

    // Only write when something actually changed: the GUI watches this entry,
    // and a write every second would wake it every second.
    if state.ringing != before.ringing
        || state.snoozed != before.snoozed
        || state.consumed_once != before.consumed_once
    {
        daemon.persist(&state);
    }
}

/// One-time import of snoozes that predate the runtime entry.
///
/// Snoozes used to live on the alarm itself as `snoozed_until`. Carry them
/// across on first run so an upgrade mid-snooze does not drop it.
fn import_legacy_snoozes(daemon: &Daemon, state: &mut RuntimeState) {
    if !state.snoozed.is_empty() {
        return;
    }
    let config = daemon.definitions();
    let now = chrono::Local::now();
    let alarms = clocks::app::persistence::restore_alarms(&config);
    state.snoozed = alarms
        .snoozed
        .into_iter()
        .filter(|s| s.retrigger_at > now)
        .map(|s| clocks::runtime::SnoozeRecord {
            alarm_id: s.alarm_id,
            label: s.label,
            sound: s.sound,
            ring_minutes: s.ring_minutes,
            snooze_minutes: s.snooze_minutes,
            retrigger_at: s.retrigger_at,
        })
        .collect();
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
    });

    import_legacy_snoozes(&daemon, &mut state);

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
