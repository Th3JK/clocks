// SPDX-License-Identifier: MIT

//! Making the daemon start at login, on the host and inside a Flatpak.
//!
//! Two mechanisms, because no single one works everywhere:
//!
//! - **D-Bus activation** starts the daemon on demand from any session bus, so
//!   it exists whenever the GUI is open. Installed as a service file; nothing
//!   here does it.
//! - **XDG autostart** extends that to "app closed", and is what this module
//!   arranges.
//!
//! Deliberately not a systemd unit: the variable across distributions is
//! systemd-vs-not (Void, Alpine, Gentoo and Artix have no systemd user
//! session), so a unit file is the *least* portable option available.
//!
//! Inside a Flatpak the sandbox cannot write `~/.config/autostart`, so the same
//! artifact has to be requested through the Background portal instead. Without
//! this a Flatpak install gets no autostart at all -- the daemon would run only
//! while the GUI had activated it, and alarms would stop with the window, which
//! is the whole problem the daemon exists to solve.

/// What the autostart entry launches.
///
/// A bare name rather than a path: inside a Flatpak it resolves within the
/// sandbox, and on the host it comes from `PATH` after `just install`. Dev
/// builds should use `just install-daemon-local`, which writes an entry
/// pointing at the built binary instead.
pub const DAEMON_COMMAND: &str = "clocks-daemon";

/// Whether we are running inside a Flatpak sandbox.
///
/// `/.flatpak-info` is present in every Flatpak sandbox; `FLATPAK_ID` covers
/// the case of being run through `flatpak-spawn` from outside one.
#[must_use]
pub fn in_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists() || std::env::var_os("FLATPAK_ID").is_some()
}

/// Ask the desktop to launch the daemon at login.
///
/// On the host this writes the autostart entry directly. Inside a Flatpak it
/// goes through `org.freedesktop.portal.Background`, which **prompts the user**
/// the first time -- so call it somewhere the reason is already on screen, not
/// at startup.
///
/// Returns `Ok(false)` when the request was understood but not granted.
/// The error is `Send + Sync` so the call can cross a `spawn_blocking`
/// boundary -- a bare `Box<dyn Error>` is neither.
pub fn request(
    daemon_command: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    if in_flatpak() {
        request_via_portal(daemon_command)
    } else {
        write_autostart_entry(daemon_command)?;
        Ok(true)
    }
}

/// Whether autostart is already arranged, so the GUI can avoid re-prompting.
///
/// Only meaningful on the host: inside a Flatpak the entry lives outside the
/// sandbox and the portal owns the answer.
#[must_use]
pub fn is_enabled() -> bool {
    !in_flatpak() && autostart_path().is_some_and(|p| p.exists())
}

fn autostart_path() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(
        dir.join("autostart")
            .join(format!("{}.Daemon.desktop", crate::APP_ID)),
    )
}

fn write_autostart_entry(command: &str) -> std::io::Result<()> {
    let Some(path) = autostart_path() else {
        return Err(std::io::Error::other("no config directory"));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &path,
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Clocks alarms\n\
             Comment=Rings alarms while Clocks is closed\n\
             Exec={command}\n\
             Icon={}\n\
             Terminal=false\n\
             NoDisplay=true\n\
             X-GNOME-Autostart-enabled=true\n",
            crate::APP_ID
        ),
    )
}

/// `org.freedesktop.portal.Background.RequestBackground`.
///
/// The portal replies asynchronously on a `Request` object rather than from the
/// call itself, so this waits for that signal instead of trusting the return.
fn request_via_portal(
    command: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    use zbus::zvariant::Value;

    let connection = zbus::blocking::Connection::session()?;

    let mut options: std::collections::HashMap<&str, Value> = std::collections::HashMap::new();
    options.insert("reason", Value::from("Ring alarms while Clocks is closed"));
    options.insert("autostart", Value::from(true));
    options.insert("background", Value::from(true));
    // Argv for the autostart entry the portal writes on our behalf.
    options.insert(
        "commandline",
        Value::from(vec![command.to_string()]),
    );

    let reply = connection.call_method(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        Some("org.freedesktop.portal.Background"),
        "RequestBackground",
        // Empty parent window: we have no exported surface handle to give it.
        &("", options),
    )?;

    let request_path: zbus::zvariant::OwnedObjectPath = reply.body().deserialize()?;

    // The result arrives as a Response signal on the request object.
    let proxy = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        request_path.as_str(),
        "org.freedesktop.portal.Request",
    )?;

    let mut signals = proxy.receive_signal("Response")?;
    let Some(signal) = signals.next() else {
        return Ok(false);
    };
    let (response, _results): (u32, std::collections::HashMap<String, zbus::zvariant::OwnedValue>) =
        signal.body().deserialize()?;

    // 0 = granted, 1 = cancelled by the user, 2 = ended some other way.
    Ok(response == 0)
}
