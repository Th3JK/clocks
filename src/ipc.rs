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
