// SPDX-License-Identifier: MIT

use std::time::Duration;

/// Format a duration as HH:MM:SS.d (always shows hours)
pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    let tenths = duration.subsec_millis() / 100;
    format!("{:02}:{:02}:{:02}.{}", hours, minutes, secs, tenths)
}

/// Split a duration into (prefix, seconds, suffix) parts for styled display.
/// prefix = "HH:MM:", seconds = "SS", suffix = ".d"
pub fn format_duration_parts(duration: Duration) -> (String, String, String) {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    let tenths = duration.subsec_millis() / 100;

    let prefix = format!("{:02}:{:02}:", hours, minutes);
    let seconds = format!("{:02}", secs);
    let suffix = format!(".{}", tenths);

    (prefix, seconds, suffix)
}

/// Format a duration as HH:MM:SS (no fractional)
pub fn format_duration_hms(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}

/// Format for a clock face: `MM:SS`, gaining an hours segment only when there
/// is one.
///
/// `format_duration_hms` always prints `HH:MM:SS`, so a five-minute chess game
/// read `00:05:00` -- eight characters of a large monospace face, which
/// overflowed the card it sat in. Conventional clock displays drop a zero hours
/// segment, and doing so also makes the common case fit.
#[must_use]
pub fn format_duration_clock(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    }
}
