// SPDX-License-Identifier: MIT
//
// Stopwatch data types: lap entries, history records, and state.

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct LapEntry {
    pub id: u32,
    pub lap_time: Duration,
    pub delta: i64,
    pub is_fastest: bool,
    pub is_slowest: bool,
}

#[derive(Debug, Clone)]
pub struct StopwatchRecord {
    pub id: u32,
    pub label: String,
    pub total_elapsed: Duration,
    pub laps: Vec<LapEntry>,
}

pub struct StopwatchState {
    pub elapsed: Duration,
    pub is_running: bool,
    pub start_instant: Option<Instant>,
    pub accumulated: Duration,
    pub laps: Vec<LapEntry>,
    pub lap_start: Duration,
    pub next_lap_id: u32,
    // History
    pub history: Vec<StopwatchRecord>,
    pub next_history_id: u32,
    pub current_label: String,
    pub current_session_id: Option<u32>,
}

impl Default for StopwatchState {
    fn default() -> Self {
        Self {
            elapsed: Duration::ZERO,
            is_running: false,
            start_instant: None,
            accumulated: Duration::ZERO,
            laps: Vec::new(),
            lap_start: Duration::ZERO,
            next_lap_id: 1,
            history: Vec::new(),
            next_history_id: 1,
            current_label: String::new(),
            current_session_id: None,
        }
    }
}

impl StopwatchState {
    pub(super) fn current_elapsed(&self) -> Duration {
        if let Some(start) = self.start_instant {
            self.accumulated + start.elapsed()
        } else {
            self.accumulated
        }
    }

    /// Build a CSV document for the entire history (oldest record first).
    pub fn history_csv(&self) -> String {
        records_to_csv(&self.history)
    }

    /// Build a CSV document for a single history record, if it exists.
    pub fn record_csv(&self, id: u32) -> Option<String> {
        self.history
            .iter()
            .find(|r| r.id == id)
            .map(std::slice::from_ref)
            .map(records_to_csv)
    }
}

/// Quote a CSV field if it contains a comma, quote, or newline (RFC 4180).
fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Render history records as CSV. One row per lap; lap-less records get a single
/// summary row with empty lap columns.
fn records_to_csv(records: &[StopwatchRecord]) -> String {
    let mut out = String::from("Session,Total Time,Lap #,Lap Time,Delta (ms)\n");
    for record in records {
        let total = crate::components::format_duration(record.total_elapsed);
        let label = csv_escape(&record.label);
        if record.laps.is_empty() {
            out.push_str(&format!("{label},{total},,,\n"));
        } else {
            for (i, lap) in record.laps.iter().enumerate() {
                let lap_time = crate::components::format_duration(lap.lap_time);
                out.push_str(&format!(
                    "{label},{total},{},{lap_time},{}\n",
                    i + 1,
                    lap.delta
                ));
            }
        }
    }
    out
}
