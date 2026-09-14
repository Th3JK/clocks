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
//! Inside a Flatpak the sandbox cannot normally write `~/.config/autostart`, so
//! the same artifact is requested through the Background portal instead --
//! *where that portal exists*. On COSMIC it does not: `cosmic.portal` declares
//! Access, FileChooser, RemoteDesktop, Screenshot, Settings and ScreenCast and
//! no `Background`, the GTK backend in the fallback chain does not implement it
//! either, and xdg-desktop-portal only exports a frontend interface when some
//! backend backs it. So on the app's own target desktop the call fails as an
//! unknown interface.
//!
//! Hence the fallback: when the portal is missing we write the entry directly,
//! which `--filesystem=xdg-config/autostart:create` in the manifest permits.
//! Without it a Flatpak install gets no autostart at all -- the daemon would run
//! only while the GUI had activated it, and alarms would stop with the window,
//! which is the whole problem the daemon exists to solve.

use std::collections::HashMap;

/// What the autostart entry launches on a host install.
///
/// A bare name rather than a path: it comes from `PATH` after `just install`.
/// Dev builds should use `just install-daemon-local`, which writes an entry
/// pointing at the built binary instead. The Flatpak form is built by
/// [`flatpak_exec`], which has to escape the sandbox to run anything.
pub const DAEMON_COMMAND: &str = "clocks-daemon";

/// How a request to start at login turned out.
///
/// Replaces a bare `bool`, which could not distinguish "the user said no" from
/// "there is no portal on this desktop" -- and so reported both as a refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Autostart {
    Enabled,
    /// The request was understood and the user declined it.
    Declined,
    /// It never got as far as being asked.
    Failed(String),
}

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
/// tries `org.freedesktop.portal.Background` first, which **prompts the user**
/// -- so call it somewhere the reason is already on screen, not at startup --
/// and writes the entry itself if that portal is not implemented here.
#[must_use]
pub fn request(daemon_command: &str) -> Autostart {
    if in_flatpak() {
        match request_via_portal() {
            Ok(true) => Autostart::Enabled,
            Ok(false) => Autostart::Declined,
            // No Background backend on this desktop. Not an error the user can
            // act on, and not a refusal -- fall back rather than report either.
            Err(PortalError::Unsupported) => match write_autostart_entry(&flatpak_exec()) {
                Ok(()) => Autostart::Enabled,
                Err(e) => Autostart::Failed(e.to_string()),
            },
            Err(PortalError::Failed(e)) => Autostart::Failed(e),
        }
    } else {
        match write_autostart_entry(daemon_command) {
            Ok(()) => Autostart::Enabled,
            Err(e) => Autostart::Failed(e.to_string()),
        }
    }
}

/// The `Exec` line for an entry that has to start a sandboxed daemon.
///
/// The entry is read by the *host* session, so it cannot name the binary
/// directly -- only `flatpak run` can reach inside. The id comes from
/// `FLATPAK_ID` rather than being hardcoded so a differently-branded build
/// still writes a working entry.
fn flatpak_exec() -> String {
    let id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| crate::APP_ID.to_string());
    format!("flatpak run --command={DAEMON_COMMAND} {id}")
}

/// Whether autostart is already arranged, so the GUI can avoid re-prompting.
///
/// Both routes land in the host's autostart directory, under different names:
/// we write `<app-id>.Daemon.desktop`, and the portal writes
/// `<app-id>.desktop`. Previously this returned `false` unconditionally inside
/// a Flatpak, so the sandboxed UI could never show the "already enabled" state
/// and kept offering the button to someone who had already granted it.
///
/// This touches the filesystem, so cache it rather than calling it from a view.
#[must_use]
pub fn is_enabled() -> bool {
    if autostart_path().is_some_and(|p| p.exists()) {
        return true;
    }
    // Only consult the portal's own entry inside a Flatpak. On the host that
    // file would be the user's "start Clocks at login" entry for the GUI, set
    // up by some other tool -- not ours, and not the daemon.
    in_flatpak() && portal_autostart_path().is_some_and(|p| p.exists())
}

/// The host's autostart directory.
///
/// **Not** `XDG_CONFIG_HOME`: inside a sandbox that points at
/// `~/.var/app/<id>/config`, so writing there would succeed, change nothing and
/// report success -- the worst of the three outcomes. `$HOME/.config` is where
/// `--filesystem=xdg-config/autostart:create` mounts the real directory, and is
/// also correct on the host whenever `XDG_CONFIG_HOME` is unset.
fn autostart_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    Some(home.join(".config").join("autostart"))
}

fn autostart_path() -> Option<std::path::PathBuf> {
    Some(autostart_dir()?.join(format!("{}.Daemon.desktop", crate::APP_ID)))
}

/// Where the Background portal writes its own entry, which is named after the
/// application id alone.
fn portal_autostart_path() -> Option<std::path::PathBuf> {
    Some(autostart_dir()?.join(format!("{}.desktop", crate::APP_ID)))
}

fn write_autostart_entry(command: &str) -> std::io::Result<()> {
    let Some(path) = autostart_path() else {
        return Err(std::io::Error::other("no home directory"));
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

enum PortalError {
    /// No backend implements `org.freedesktop.impl.portal.Background` here, so
    /// the frontend interface is not exported at all.
    Unsupported,
    Failed(String),
}

/// `org.freedesktop.portal.Background.RequestBackground`.
///
/// The portal replies asynchronously on a `Request` object rather than from the
/// call itself, so this waits for that signal instead of trusting the return.
fn request_via_portal() -> Result<bool, PortalError> {
    use zbus::zvariant::Value;

    let connection =
        zbus::blocking::Connection::session().map_err(|e| PortalError::Failed(e.to_string()))?;

    // The request path is derived from the token, which lets us subscribe
    // before making the call. Subscribing afterwards races the reply: a portal
    // that answers immediately emits Response before the match rule exists, and
    // the wait then blocks forever.
    let token = format!("clocks_{}", std::process::id());
    let unique = connection
        .unique_name()
        .map(|n| n.trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default();
    let request_path = format!("/org/freedesktop/portal/desktop/request/{unique}/{token}");

    // `classify` here too: if there is no portal service at all the builder
    // fails on name resolution, which is the same "nothing to ask" case as a
    // missing interface and should fall back rather than report an error.
    let request = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        request_path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .map_err(classify)?;
    let mut signals = request
        .receive_signal("Response")
        .map_err(|e| PortalError::Failed(e.to_string()))?;

    // Exactly the keys the interface accepts: handle_token, reason, autostart,
    // commandline, dbus-activatable. `background` is a field of the *reply*,
    // not an option -- passing it is a protocol error.
    //
    // `commandline` rather than `dbus-activatable`, because activation targets
    // the application id, which is the GUI. We want the daemon.
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("reason", Value::from(crate::fl!("autostart-reason")));
    options.insert("autostart", Value::from(true));
    options.insert(
        "commandline",
        Value::from(vec![DAEMON_COMMAND.to_string()]),
    );

    let reply = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Background"),
            "RequestBackground",
            // Empty parent window: we have no exported surface handle to give it.
            &("", options),
        )
        .map_err(classify)?;

    // The portal is free to pick a different path than the one we derived. If
    // it did, the subscription above is on the wrong object and waiting on it
    // would hang, so move to the real one -- accepting the small race that
    // deriving the path in advance exists to avoid.
    let actual: zbus::zvariant::OwnedObjectPath = reply
        .body()
        .deserialize()
        .map_err(|e| PortalError::Failed(e.to_string()))?;
    if actual.as_str() != request_path {
        let request = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            actual.as_str(),
            "org.freedesktop.portal.Request",
        )
        .map_err(|e| PortalError::Failed(e.to_string()))?;
        signals = request
            .receive_signal("Response")
            .map_err(|e| PortalError::Failed(e.to_string()))?;
    }

    let Some(signal) = signals.next() else {
        return Err(PortalError::Failed("portal closed without replying".into()));
    };
    let (response, _results): (u32, HashMap<String, zbus::zvariant::OwnedValue>) = signal
        .body()
        .deserialize()
        .map_err(|e| PortalError::Failed(e.to_string()))?;

    // 0 = granted, 1 = cancelled by the user, 2 = ended some other way.
    Ok(response == 0)
}

/// Tell "this desktop has no Background portal" apart from a real failure.
///
/// Which of these three a missing backend produces depends on the portal
/// version -- the frontend may not export the interface, may not own the name
/// yet, or may reject the member -- so all three are treated as unsupported.
fn classify(error: zbus::Error) -> PortalError {
    if let zbus::Error::MethodError(name, _, _) = &error {
        let name = name.as_str();
        if matches!(
            name,
            "org.freedesktop.DBus.Error.UnknownMethod"
                | "org.freedesktop.DBus.Error.UnknownInterface"
                | "org.freedesktop.DBus.Error.UnknownObject"
                | "org.freedesktop.DBus.Error.ServiceUnknown"
                | "org.freedesktop.DBus.Error.NameHasNoOwner"
        ) {
            return PortalError::Unsupported;
        }
    }
    PortalError::Failed(error.to_string())
}
