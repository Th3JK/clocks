// SPDX-License-Identifier: MIT
//
// Time-format preference and the one place wall-clock times get formatted.
//
// Before this module each page rolled its own 12/24-hour branch; six of those
// used chrono's `%p`, which emits hardcoded English AM/PM regardless of locale.
// Everything now goes through `format_time_of_day`.

use crate::fl;
use chrono::Timelike;
use cosmic_config::ConfigGet;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeFormat {
    /// Follow the desktop, falling back to the locale. See `resolve_use_12h`.
    System,
    TwentyFour,
    Twelve,
}

impl TimeFormat {
    /// Resolve to a concrete 12-hour flag.
    pub fn use_12h(self) -> bool {
        match self {
            TimeFormat::TwentyFour => false,
            TimeFormat::Twelve => true,
            TimeFormat::System => system_use_12h(),
        }
    }
}

/// The desktop's own preference, with a locale fallback.
///
/// 1. COSMIC Settings → Time & Date writes `military_time` into the time
///    applet's config; this is what the panel clock reads. It is not exposed as
///    a libcosmic API, so it is read as a foreign app's config id.
/// 2. Outside COSMIC — or before the user has ever touched the setting, since
///    there is no packaged system default to fall back on — use the locale's
///    own time format.
/// 3. Failing both, 24-hour.
///
/// Cached: neither source changes without a restart of the relevant daemon, and
/// this is called from view code that runs every frame.
fn system_use_12h() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        if let Some(military) = cosmic_military_time() {
            return !military;
        }
        locale_use_12h().unwrap_or(false)
    })
}

/// `military_time` from `com.system76.CosmicAppletTime`. `true` means 24-hour.
fn cosmic_military_time() -> Option<bool> {
    let config = cosmic_config::Config::new("com.system76.CosmicAppletTime", 1).ok()?;
    config.get::<bool>("military_time").ok()
}

/// Whether the current locale's time format is 12-hour.
///
/// `nl_langinfo(T_FMT)` returns the locale's time format string; a 12-hour
/// locale uses `%I` (12-hour clock) and/or `%p` (AM/PM). `setlocale` must run
/// first or glibc answers for the "C" locale, which is always 24-hour.
fn locale_use_12h() -> Option<bool> {
    // SAFETY: `setlocale` and `nl_langinfo` mutate and read process-global
    // locale state, so neither is thread-safe. Both are called exactly once,
    // behind the `OnceLock` in `system_use_12h`, before any other thread could
    // touch the locale. The returned pointer is owned by libc and only read
    // here, before any further locale call could invalidate it.
    unsafe {
        if libc::setlocale(libc::LC_TIME, c"".as_ptr()).is_null() {
            return None;
        }
        let fmt = libc::nl_langinfo(libc::T_FMT);
        if fmt.is_null() {
            return None;
        }
        let fmt = std::ffi::CStr::from_ptr(fmt).to_string_lossy();
        Some(fmt.contains("%p") || fmt.contains("%I"))
    }
}

/// Format an hour/minute pair, translated.
pub fn format_hm(hour: u32, minute: u32, use_12h: bool) -> String {
    if use_12h {
        let (h, is_pm) = to_12h(hour);
        let period = if is_pm { fl!("pm") } else { fl!("am") };
        format!("{h:02}:{minute:02} {period}")
    } else {
        format!("{hour:02}:{minute:02}")
    }
}

/// Format a timestamp's time of day, translated.
pub fn format_time_of_day<T: Timelike>(t: &T, use_12h: bool) -> String {
    format_hm(t.hour(), t.minute(), use_12h)
}

/// As `format_time_of_day`, but including seconds.
pub fn format_time_of_day_secs<T: Timelike>(t: &T, use_12h: bool) -> String {
    if use_12h {
        let (h, is_pm) = to_12h(t.hour());
        let period = if is_pm { fl!("pm") } else { fl!("am") };
        format!("{:02}:{:02}:{:02} {}", h, t.minute(), t.second(), period)
    } else {
        format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second())
    }
}

/// 24-hour hour to (12-hour hour, is_pm). Midnight and noon are both 12.
pub fn to_12h(hour24: u32) -> (u32, bool) {
    let is_pm = hour24 >= 12;
    let h = hour24 % 12;
    (if h == 0 { 12 } else { h }, is_pm)
}
