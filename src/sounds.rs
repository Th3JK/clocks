// SPDX-License-Identifier: MIT
//
// Sound names, kept out of the widget layer.
//
// These live here rather than next to the dropdown that renders them because
// `audio` needs them to tell a built-in name from a custom file path, and the
// background daemon needs `audio` without dragging in libcosmic's widget tree.

/// Available notification sounds (built-in options + "Custom..." at the end)
pub const SOUND_OPTIONS: &[&str] = &[
    "Bell",
    "Chime",
    "Alert",
    "Gentle",
    "Custom...",
];

/// Find the dropdown index for a given sound name.
/// Returns the index in SOUND_OPTIONS, or None if it's a custom path.
pub fn sound_option_index(current: &str) -> Option<usize> {
    SOUND_OPTIONS.iter().position(|&s| s == current)
}
