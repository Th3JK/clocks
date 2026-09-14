// SPDX-License-Identifier: MIT

//! Startup flags, and the single-instance activation payload.
//!
//! Doubles as the mechanism for "clicking a notification opens the app on the
//! right page": libcosmic's `run_single_instance` forwards an action and its
//! arguments to an already-running instance over D-Bus, and starts one if there
//! is none. So the daemon simply launches `clocks <page-key>` and lets libcosmic
//! decide whether that means focusing the existing window or opening a new one.

use crate::pages::Page;

/// The only action we forward. `CosmicFlags::SubCommand` requires `ToString`,
/// and this is what lands in `Details::ActivateAction { action, .. }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowPage;

impl std::fmt::Display for ShowPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("show-page")
    }
}

#[derive(Debug, Clone, Default)]
pub struct Flags {
    /// Page to open on, if one was named. `None` means "wherever you were".
    pub page: Option<Page>,
}

impl Flags {
    /// Parse the process arguments. Deliberately tiny -- the app takes one
    /// optional positional page key and nothing else, so a full CLI parser
    /// would be more machinery than it earns.
    #[must_use]
    pub fn from_args() -> Self {
        Self {
            page: std::env::args().nth(1).as_deref().and_then(Page::from_key),
        }
    }
}

impl cosmic::app::CosmicFlags for Flags {
    type SubCommand = ShowPage;
    type Args = Vec<String>;

    fn action(&self) -> Option<&Self::SubCommand> {
        // Only forward when there is actually a page to show; otherwise let
        // libcosmic send a plain Activate, which just raises the window.
        self.page.is_some().then_some(&ShowPage)
    }

    fn args(&self) -> Vec<&str> {
        self.page.map(Page::key).into_iter().collect()
    }
}
