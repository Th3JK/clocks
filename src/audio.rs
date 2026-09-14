// SPDX-License-Identifier: MIT

use crate::sounds::SOUND_OPTIONS;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Resolve a sound name to a file path
pub fn resolve_sound_path(sound: &str) -> Option<String> {
    // Custom file path (not a built-in option)
    if !SOUND_OPTIONS.contains(&sound) && !sound.is_empty() {
        return Some(sound.to_string());
    }

    let filename = match sound {
        "Bell" => "bell",
        "Chime" => "chime",
        "Alert" => "alert",
        "Gentle" => "gentle",
        _ => "bell", // fallback
    };

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let candidates = [
        // Alongside the binary, for a portable/extracted layout.
        exe_dir
            .as_ref()
            .map(|d| d.join(format!("resources/audio/{}.wav", filename))),
        // `<prefix>/share/clocks/audio`, derived from the binary rather than
        // baked in, so it resolves for any prefix -- /usr, /usr/local, and
        // /app inside a Flatpak all work without being special-cased.
        exe_dir
            .as_ref()
            .and_then(|d| d.parent())
            .map(|p| p.join(format!("share/clocks/audio/{}.wav", filename))),
        // The build tree, for `cargo run`.
        Some(std::path::PathBuf::from(format!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/resources/audio/{}.wav"),
            filename
        ))),
    ];
    candidates
        .iter()
        .flatten()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
}

/// Play a sound once (for timer/pomodoro completions)
pub fn play_sound(sound: &str) {
    let Some(path) = resolve_sound_path(sound) else {
        eprintln!("Sound file not found for: {}", sound);
        return;
    };

    std::thread::spawn(move || {
        if let Err(e) = play_sound_file(&path) {
            eprintln!("Failed to play sound {}: {}", path, e);
        }
    });
}

fn play_sound_file(path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rodio::{Decoder, DeviceSinkBuilder, Source};

    let file = std::fs::File::open(path)?;
    let mut sink = DeviceSinkBuilder::open_default_sink()?;
    sink.log_on_drop(false);
    let source = Decoder::try_from(file)?;
    let duration = source.total_duration().unwrap_or(Duration::from_secs(5));
    sink.mixer().add(source);
    std::thread::sleep(duration.min(Duration::from_secs(10)));
    Ok(())
}

/// Play alarm sound in a loop for the given ring duration, with fade-out 3s before end.
/// Stops early if the stop flag is set.
pub fn play_alarm_sound_loop(
    sound: &str,
    ring_secs: u64,
    stop: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rodio::{Decoder, DeviceSinkBuilder, Player, Source};

    let Some(path) = resolve_sound_path(sound) else {
        return Err("Sound file not found".into());
    };

    let mut sink = DeviceSinkBuilder::open_default_sink()?;
    sink.log_on_drop(false);
    let player = Player::connect_new(sink.mixer());

    // Load and loop the source
    let file = std::fs::File::open(&path)?;
    let source = Decoder::try_from(file)?.repeat_infinite();
    player.append(source);

    let start = std::time::Instant::now();
    let ring_duration = Duration::from_secs(ring_secs);
    let fade_start = ring_duration.saturating_sub(Duration::from_secs(3));

    loop {
        if stop.load(Ordering::Relaxed) {
            player.stop();
            return Ok(());
        }

        let elapsed = start.elapsed();
        if elapsed >= ring_duration {
            player.stop();
            return Ok(());
        }

        // Fade out during the last 3 seconds
        if elapsed >= fade_start {
            let fade_remaining = ring_duration.saturating_sub(elapsed);
            let volume = fade_remaining.as_secs_f32() / 3.0;
            player.set_volume(volume.clamp(0.0, 1.0));
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Send a desktop notification
pub fn send_notification(title: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .icon("alarm-symbolic")
        .timeout(notify_rust::Timeout::Milliseconds(5000))
        .show();
}
