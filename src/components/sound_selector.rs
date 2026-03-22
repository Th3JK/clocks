// SPDX-License-Identifier: MIT

use cosmic::widget;
use std::borrow::Cow;

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

/// Build a sound selector dropdown widget.
/// `on_select` is called with the sound name (for built-in) or `on_custom` for custom file browsing.
pub fn sound_selector_view<'a, M: Clone + 'static + Send + Sync>(
    label: String,
    current: &str,
    on_select: impl Fn(String) -> M + Send + Sync + 'static,
    on_custom: M,
) -> cosmic::Element<'a, M> {
    let is_custom_path = sound_option_index(current).is_none() && !current.is_empty();

    // Build the list of options for the dropdown, appending custom filename if set
    let mut options: Vec<String> = SOUND_OPTIONS.iter().map(|s| s.to_string()).collect();

    let selected = if is_custom_path {
        // Insert the custom filename before "Custom..." (which is the last entry)
        let display_name = std::path::Path::new(current)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(current)
            .to_string();
        let insert_pos = options.len() - 1;
        options.insert(insert_pos, display_name);
        Some(insert_pos)
    } else {
        sound_option_index(current)
    };

    let custom_path = current.to_string();
    let custom_insert = is_custom_path;

    let mut col = widget::column::with_capacity(2)
        .spacing(6);

    col = col.push(widget::text::body(label));

    col = col.push(
        widget::dropdown(
            Cow::Owned(options.clone()),
            selected,
            move |idx| {
                let last = options.len() - 1;
                if idx == last {
                    // "Custom..." is always the last entry
                    on_custom.clone()
                } else if custom_insert && idx == last - 1 {
                    // Re-selected the current custom file
                    on_select(custom_path.clone())
                } else {
                    on_select(SOUND_OPTIONS[idx].to_string())
                }
            },
        )
        .width(cosmic::iced::Length::Fill),
    );

    col.into()
}
