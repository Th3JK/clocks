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
