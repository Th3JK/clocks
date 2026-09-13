// SPDX-License-Identifier: MIT
//
// Parser for the quick-action palette.
//
// Deliberately small and predictable rather than clever: a leading or trailing
// keyword picks the verb, one token supplies the value, and whatever is left
// becomes the label. Anything that does not parse falls back to page navigation
// so the palette is never a dead end.
//
// This is the only free-text input in the app — every other numeric field is a
// stepper — so it is also the only place worth unit-testing.

use crate::pages::Page;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickAction {
    /// Create (and start) a timer of this many seconds.
    Timer { secs: u64, label: Option<String> },
    /// Create an enabled one-shot alarm at this 24-hour time.
    Alarm {
        hour: u32,
        minute: u32,
        label: Option<String>,
    },
    /// Create a countdown event at this date, 09:00 local.
    Countdown {
        year: i32,
        month: u32,
        day: u32,
        label: Option<String>,
    },
    /// Add a world clock for the first timezone matching this text.
    Clock { query: String },
    /// Jump to a page.
    Navigate(Page),
    // Launchers for items the user has already saved. These carry ids rather
    // than values, so their descriptions have to be resolved against app state.
    StartTimer(u32),
    StartPomodoro(u32),
    StartWorkout(u32),
}

/// Every page and the words that select it. Matched by prefix, so "pom" works.
fn page_keywords() -> [(Page, &'static [&'static str]); 8] {
    [
        (Page::WorldClocks, &["world", "clocks", "worldclocks"]),
        (Page::Stopwatch, &["stopwatch"]),
        (Page::Alarm, &["alarms"]),
        (Page::Timer, &["timers"]),
        (Page::Pomodoro, &["pomodoro"]),
        (Page::Chess, &["chess"]),
        (Page::Workout, &["workout"]),
        (Page::Countdown, &["countdowns"]),
    ]
}

/// Actions offered when the palette opens, before anything is typed.
///
/// Common timer lengths first, then every page, so the palette doubles as a
/// page jumper and is useful without knowing the grammar.
pub fn presets() -> Vec<QuickAction> {
    let timers = [60, 5 * 60, 10 * 60, 15 * 60, 25 * 60, 60 * 60];
    timers
        .into_iter()
        .map(|secs| QuickAction::Timer { secs, label: None })
        .chain(
            page_keywords()
                .into_iter()
                .map(|(page, _)| QuickAction::Navigate(page)),
        )
        .collect()
}

/// Parse a palette query. `None` means nothing sensible matched.
pub fn parse(input: &str) -> Option<QuickAction> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let lower = input.to_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();

    // Verb first: a keyword anywhere in the query selects the action, then the
    // remaining tokens supply the value and the label. Checking the verb before
    // the value is what lets "5m timer" and "timer 5m" both work.
    if let Some(rest) = strip_keyword(&tokens, "timer") {
        let (secs, label) = take_first(&rest, parse_duration)?;
        return Some(QuickAction::Timer {
            secs,
            label: rebuild_label(input, &label),
        });
    }

    if let Some(rest) = strip_keyword(&tokens, "alarm") {
        let ((hour, minute), label) = take_first(&rest, parse_time)?;
        return Some(QuickAction::Alarm {
            hour,
            minute,
            label: rebuild_label(input, &label),
        });
    }

    if let Some(rest) = strip_keyword(&tokens, "countdown") {
        let ((year, month, day), label) = take_first(&rest, parse_date)?;
        return Some(QuickAction::Countdown {
            year,
            month,
            day,
            label: rebuild_label(input, &label),
        });
    }

    if let Some(rest) = strip_keyword(&tokens, "clock") {
        let query = rest.join(" ");
        if !query.is_empty() {
            return Some(QuickAction::Clock { query });
        }
    }

    // A bare duration is the most common thing anyone types, so treat it as a
    // timer even without the keyword.
    if tokens.len() == 1
        && let Some(secs) = parse_duration(tokens[0])
    {
        return Some(QuickAction::Timer { secs, label: None });
    }

    // Fall back to navigation.
    page_match(&lower).map(QuickAction::Navigate)
}

/// A page whose name starts with this query, so "pom" reaches Pomodoro.
pub fn page_match(query: &str) -> Option<Page> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    page_keywords()
        .into_iter()
        .find(|(_, words)| words.iter().any(|w| w.starts_with(&q) || *w == q))
        .map(|(page, _)| page)
}

/// Remove the first token equal to `keyword`, returning the rest. `None` when
/// the keyword is absent.
fn strip_keyword<'a>(tokens: &[&'a str], keyword: &str) -> Option<Vec<&'a str>> {
    let pos = tokens.iter().position(|t| *t == keyword)?;
    let mut rest = tokens.to_vec();
    rest.remove(pos);
    Some(rest)
}

/// Apply `f` to each token, returning the first success plus the tokens that
/// did not match (in order) as the label.
fn take_first<'a, T>(tokens: &[&'a str], f: impl Fn(&str) -> Option<T>) -> Option<(T, Vec<&'a str>)> {
    let mut value = None;
    let mut rest = Vec::new();
    for token in tokens {
        match (&value, f(token)) {
            (None, Some(v)) => value = Some(v),
            _ => rest.push(*token),
        }
    }
    value.map(|v| (v, rest))
}

/// Recover the label with its original casing. The tokens were lowercased for
/// matching, so they are looked back up in the untouched input.
fn rebuild_label(original: &str, tokens: &[&str]) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }
    let wanted: Vec<String> = tokens.iter().map(|t| t.to_string()).collect();
    let kept: Vec<&str> = original
        .split_whitespace()
        .filter(|w| wanted.contains(&w.to_lowercase()))
        .collect();
    let label = kept.join(" ");
    (!label.is_empty()).then_some(label)
}

/// `5m`, `90s`, `1h30m`, `2h`, or a bare number meaning minutes.
pub fn parse_duration(token: &str) -> Option<u64> {
    if token.is_empty() {
        return None;
    }
    // Bare number: minutes, which is what people mean by "5".
    if let Ok(mins) = token.parse::<u64>() {
        return (mins > 0).then_some(mins * 60);
    }

    let mut total = 0u64;
    let mut digits = String::new();
    let mut saw_unit = false;
    for ch in token.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        let value: u64 = digits.parse().ok()?;
        digits.clear();
        total += match ch {
            'h' => value * 3600,
            'm' => value * 60,
            's' => value,
            _ => return None,
        };
        saw_unit = true;
    }
    // Trailing digits with no unit ("1h30") are ambiguous; reject rather than guess.
    (saw_unit && digits.is_empty() && total > 0).then_some(total)
}

/// `6:30`, `18:00`, `7am`, `6:30pm`, or a bare hour.
pub fn parse_time(token: &str) -> Option<(u32, u32)> {
    let (body, meridiem) = if let Some(b) = token.strip_suffix("am") {
        (b, Some(false))
    } else if let Some(b) = token.strip_suffix("pm") {
        (b, Some(true))
    } else {
        (token, None)
    };
    let body = body.trim();
    if body.is_empty() {
        return None;
    }

    let (hour, minute) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        // A bare number is only a time when it came with am/pm or is a
        // plausible hour; otherwise `parse_duration` should have claimed it.
        None => (body.parse::<u32>().ok()?, 0),
    };
    if minute > 59 {
        return None;
    }

    let hour = match meridiem {
        // 12am is 00:00 and 12pm is 12:00.
        Some(true) => {
            if hour > 12 {
                return None;
            }
            if hour == 12 { 12 } else { hour + 12 }
        }
        Some(false) => {
            if hour > 12 {
                return None;
            }
            if hour == 12 { 0 } else { hour }
        }
        None => hour,
    };
    (hour < 24).then_some((hour, minute))
}

/// `YYYY-MM-DD`.
pub fn parse_date(token: &str) -> Option<(i32, u32, u32)> {
    let mut parts = token.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("5m"), Some(300));
        assert_eq!(parse_duration("90s"), Some(90));
        assert_eq!(parse_duration("1h30m"), Some(5400));
        assert_eq!(parse_duration("2h"), Some(7200));
        assert_eq!(parse_duration("5"), Some(300), "bare number means minutes");
        assert_eq!(parse_duration("1h30"), None, "trailing unitless digits");
        assert_eq!(parse_duration("0"), None);
        assert_eq!(parse_duration("abc"), None);
    }

    #[test]
    fn times() {
        assert_eq!(parse_time("6:30"), Some((6, 30)));
        assert_eq!(parse_time("18:00"), Some((18, 0)));
        assert_eq!(parse_time("7am"), Some((7, 0)));
        assert_eq!(parse_time("7pm"), Some((19, 0)));
        assert_eq!(parse_time("12am"), Some((0, 0)), "midnight");
        assert_eq!(parse_time("12pm"), Some((12, 0)), "noon");
        assert_eq!(parse_time("6:30pm"), Some((18, 30)));
        assert_eq!(parse_time("25:00"), None);
        assert_eq!(parse_time("6:75"), None);
        assert_eq!(parse_time("13pm"), None);
    }

    #[test]
    fn dates() {
        assert_eq!(parse_date("2026-12-25"), Some((2026, 12, 25)));
        assert_eq!(parse_date("2026-13-01"), None);
        assert_eq!(parse_date("2026-12"), None);
    }

    #[test]
    fn timer_either_word_order() {
        let expected = QuickAction::Timer {
            secs: 300,
            label: None,
        };
        assert_eq!(parse("5m timer"), Some(expected.clone()));
        assert_eq!(parse("timer 5m"), Some(expected));
    }

    #[test]
    fn timer_keeps_label_casing() {
        assert_eq!(
            parse("timer 10m Tea Brewing"),
            Some(QuickAction::Timer {
                secs: 600,
                label: Some("Tea Brewing".to_string()),
            })
        );
    }

    #[test]
    fn bare_duration_is_a_timer() {
        assert_eq!(
            parse("5m"),
            Some(QuickAction::Timer {
                secs: 300,
                label: None
            })
        );
    }

    #[test]
    fn alarms() {
        assert_eq!(
            parse("alarm 6"),
            Some(QuickAction::Alarm {
                hour: 6,
                minute: 0,
                label: None
            })
        );
        assert_eq!(
            parse("alarm 6:30 Wake up"),
            Some(QuickAction::Alarm {
                hour: 6,
                minute: 30,
                label: Some("Wake up".to_string())
            })
        );
    }

    #[test]
    fn countdown_and_clock() {
        assert_eq!(
            parse("countdown 2026-12-25 Christmas"),
            Some(QuickAction::Countdown {
                year: 2026,
                month: 12,
                day: 25,
                label: Some("Christmas".to_string())
            })
        );
        assert_eq!(
            parse("clock tokyo"),
            Some(QuickAction::Clock {
                query: "tokyo".to_string()
            })
        );
    }

    #[test]
    fn falls_back_to_navigation() {
        assert_eq!(parse("pom"), Some(QuickAction::Navigate(Page::Pomodoro)));
        assert_eq!(parse("chess"), Some(QuickAction::Navigate(Page::Chess)));
        assert_eq!(parse("qqq"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn incomplete_verbs_do_not_match() {
        assert_eq!(parse("timer"), None, "no duration given");
        assert_eq!(parse("alarm"), None, "no time given");
    }
}
