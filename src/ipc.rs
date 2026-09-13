// SPDX-License-Identifier: MIT

//! The GUI <-> daemon interface.
//!
//! Deliberately small. Commands travel over D-Bus; *state* travels through the
//! `RuntimeState` config entry, which the GUI already watches. That split means
//! the GUI needs no async D-Bus client and no signal subscription -- it calls a
//! method and the resulting state change arrives through the watcher it was
//! going to have anyway.

/// Well-known bus name. Distinct from the name libcosmic's `single-instance`
/// feature claims for the GUI, which is [`crate::APP_ID`] itself.
pub const DAEMON_BUS_NAME: &str = "dev.th3jk.clocks.Daemon";
pub const DAEMON_PATH: &str = "/dev/th3jk/clocks/Daemon";
pub const DAEMON_INTERFACE: &str = "dev.th3jk.clocks.Daemon";

/// Call a no-argument-plus-id method on the daemon, starting it if it is not
/// already running.
///
/// Blocking, so callers on an async runtime should hand it to
/// `tokio::task::spawn_blocking`. Errors are returned rather than logged: a
/// failure here means the daemon could not be reached at all, which the caller
/// may want to surface.
fn call_with_id(method: &str, alarm_id: u32) -> Result<(), zbus::Error> {
    let connection = zbus::blocking::Connection::session()?;
    connection.call_method(
        Some(DAEMON_BUS_NAME),
        DAEMON_PATH,
        Some(DAEMON_INTERFACE),
        method,
        &(alarm_id),
    )?;
    Ok(())
}

/// Stop an alarm ringing.
pub fn dismiss(alarm_id: u32) -> Result<(), zbus::Error> {
    call_with_id("Dismiss", alarm_id)
}

/// Stop an alarm ringing and re-ring after its snooze interval.
pub fn snooze(alarm_id: u32) -> Result<(), zbus::Error> {
    call_with_id("Snooze", alarm_id)
}

/// Start a timer from its full duration.
pub fn timer_start(timer_id: u32) -> Result<(), zbus::Error> {
    call_with_id("TimerStart", timer_id)
}

/// Pause a running timer, keeping the remainder.
pub fn timer_pause(timer_id: u32) -> Result<(), zbus::Error> {
    call_with_id("TimerPause", timer_id)
}

/// Resume a paused timer from where it stopped.
pub fn timer_resume(timer_id: u32) -> Result<(), zbus::Error> {
    call_with_id("TimerResume", timer_id)
}

/// Stop a timer and forget its progress.
pub fn timer_reset(timer_id: u32) -> Result<(), zbus::Error> {
    call_with_id("TimerReset", timer_id)
}

/// Start a pomodoro from the beginning of a work session.
pub fn pomodoro_start(timer_id: u32) -> Result<(), zbus::Error> {
    call_with_id("PomodoroStart", timer_id)
}

pub fn pomodoro_pause(timer_id: u32) -> Result<(), zbus::Error> {
    call_with_id("PomodoroPause", timer_id)
}

pub fn pomodoro_resume(timer_id: u32) -> Result<(), zbus::Error> {
    call_with_id("PomodoroResume", timer_id)
}

/// Jump to the next phase without waiting it out.
pub fn pomodoro_skip(timer_id: u32) -> Result<(), zbus::Error> {
    call_with_id("PomodoroSkip", timer_id)
}

pub fn pomodoro_reset(timer_id: u32) -> Result<(), zbus::Error> {
    call_with_id("PomodoroReset", timer_id)
}

/// Tell the daemon the GUI has written `secs` into daily stats, so it can stop
/// banking them. Daily stats live in the GUI-owned config entry, so this is the
/// handshake that keeps a single writer per entry.
pub fn pomodoro_focus_recorded(timer_id: u32, secs: u64) -> Result<(), zbus::Error> {
    let connection = zbus::blocking::Connection::session()?;
    connection.call_method(
        Some(DAEMON_BUS_NAME),
        DAEMON_PATH,
        Some(DAEMON_INTERFACE),
        "PomodoroFocusRecorded",
        &(timer_id, secs),
    )?;
    Ok(())
}

/// Ask the daemon to re-read alarm definitions.
///
/// The daemon watches the config itself, so this is only a nudge for the case
/// where the watcher missed an edit -- and, usefully, it is what activates the
/// daemon in the first place when the GUI starts.
pub fn reload() -> Result<(), zbus::Error> {
    let connection = zbus::blocking::Connection::session()?;
    connection.call_method(
        Some(DAEMON_BUS_NAME),
        DAEMON_PATH,
        Some(DAEMON_INTERFACE),
        "Reload",
        &(),
    )?;
    Ok(())
}
