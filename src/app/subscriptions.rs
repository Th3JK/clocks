// SPDX-License-Identifier: MIT
//
// Subscription and input-handling functions: tick timer, keyboard shortcuts,
// and the file-chooser dialog for custom sounds.

use super::{CustomSoundTarget, Message};
use cosmic::iced::keyboard::{self, key::Named, Key};
use cosmic::iced_futures;
use cosmic::prelude::*;
use std::time::Duration;

pub(super) fn tick_subscription() -> impl futures_util::Stream<Item = Message> {
    use futures_util::SinkExt;
    iced_futures::stream::channel(1, async |mut emitter| {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            _ = emitter.send(Message::Tick).await;
        }
    })
}

pub(super) fn input_subscription(
    event: cosmic::iced::Event,
    status: cosmic::iced::event::Status,
    _window: cosmic::iced::window::Id,
) -> Option<Message> {
    // Only handle ignored events (not consumed by widgets like text inputs)
    if status != cosmic::iced::event::Status::Ignored {
        return None;
    }

    let cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
        key, modifiers, ..
    }) = event
    else {
        return None;
    };

    let ctrl = modifiers.control();
    let alt = modifiers.alt();

    // For non-alphabetic character keys, Shift is inherent to producing the
    // character (e.g. Shift+/ → ?), so strip it to avoid mismatches when a
    // binding is defined for the shifted character without an explicit Shift
    // modifier.
    let shift = match key.as_ref() {
        Key::Character(c) if c.chars().all(|ch| !ch.is_ascii_alphabetic()) => false,
        _ => modifiers.shift(),
    };

    match key.as_ref() {
        // Ctrl+Q → Quit
        Key::Character("q") if ctrl && !alt && !shift => Some(Message::Quit),

        // Ctrl+PageDown → Next tab
        Key::Named(Named::PageDown) if ctrl && !alt && !shift => Some(Message::NavigateNext),

        // Ctrl+PageUp → Previous tab
        Key::Named(Named::PageUp) if ctrl && !alt && !shift => Some(Message::NavigatePrev),

        // Alt+1..5 → Direct tab navigation
        Key::Character("1") if alt && !ctrl && !shift => Some(Message::NavigateTo(0)),
        Key::Character("2") if alt && !ctrl && !shift => Some(Message::NavigateTo(1)),
        Key::Character("3") if alt && !ctrl && !shift => Some(Message::NavigateTo(2)),
        Key::Character("4") if alt && !ctrl && !shift => Some(Message::NavigateTo(3)),
        Key::Character("5") if alt && !ctrl && !shift => Some(Message::NavigateTo(4)),

        // Ctrl+N → New item (page-scoped)
        Key::Character("n") if ctrl && !alt && !shift => Some(Message::PageShortcutCtrlN),

        // Ctrl+? (Ctrl+Shift+/) → Show shortcuts dialog
        Key::Character("?") if ctrl && !alt => Some(Message::ShowShortcutsDialog),
        // Fallback: some platforms report the physical key "/" instead of "?"
        Key::Character("/") if ctrl && !alt && modifiers.shift() => {
            Some(Message::ShowShortcutsDialog)
        }

        // Ctrl+S → Skip break (Pomodoro)
        Key::Character("s") if ctrl && !alt && !shift => Some(Message::PageShortcutSkip),

        // Space → Start/Pause (page-scoped)
        Key::Character(" ") if !ctrl && !alt && !shift => Some(Message::PageShortcutSpace),

        // Enter → Lap (page-scoped)
        Key::Named(Named::Enter) if !ctrl && !alt && !shift => Some(Message::PageShortcutEnter),

        // Delete / Backspace → Reset (page-scoped)
        Key::Named(Named::Delete | Named::Backspace) if !ctrl && !alt && !shift => {
            Some(Message::PageShortcutDelete)
        }

        // Escape → Close shortcuts dialog
        Key::Named(Named::Escape) => Some(Message::CloseShortcutsDialog),

        _ => None,
    }
}

pub(super) fn open_sound_file_dialog(
    target: CustomSoundTarget,
) -> Task<cosmic::Action<Message>> {
    cosmic::task::future(async move {
        let dialog = cosmic::dialog::file_chooser::open::Dialog::new()
            .title(crate::fl!("choose-sound-file"));

        match dialog.open_file().await {
            Ok(response) => {
                let path = response.url().path().to_string();
                cosmic::Action::App(Message::CustomSoundSelected(target, path))
            }
            Err(_) => cosmic::Action::App(Message::Tick),
        }
    })
}
